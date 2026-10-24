// Structured HTTP/1.1-friendly client using curl, with status and headers.
// Returns Babel's recoverable Data result; no shell command invocation.
fn babel_http_request_data(method: String, url: String, options: BabelData) -> BabelData {
    let result = (|| -> Result<BabelData, String> {
        if !babel_permission_allowed("network") {
            return Err("Babel capability denied: network".into());
        }
        if !matches!(method.as_str(), "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD") {
            return Err("Unsupported HTTP method".into());
        }
        if url.len() > 4096 || !(url.starts_with("https://") || url.starts_with("http://"))
            || url.bytes().any(|b| b.is_ascii_control()) {
            return Err("Invalid HTTP(S) URL".into());
        }
        let BabelData::Object(fields) = options else {
            return Err("HTTP options must be a Data record".into());
        };
        for name in fields.keys() {
            if !matches!(name.as_str(), "headers" | "body" | "timeout" | "redirects") {
                return Err(format!("Unknown HTTP option: {name}"));
            }
        }
        let timeout = match fields.get("timeout") {
            None => 30u64,
            Some(BabelData::Number(s)) => {
                let n = s.parse::<u64>().map_err(|_| "Timeout must be an integer")?;
                if n == 0 || n > 60 { return Err("Timeout must be between 1 and 60 seconds".into()); }
                n
            },
            _ => return Err("Timeout must be numeric".into()),
        };
        let redirects = match fields.get("redirects") {
            None => false,
            Some(BabelData::Bool(flag)) => *flag,
            _ => return Err("redirects must be a Boolean".into()),
        };
        let body = match fields.get("body") {
            None => String::new(),
            Some(BabelData::Text(s)) if s.len() <= 4 * 1024 * 1024 => s.clone(),
            Some(BabelData::Text(_)) => return Err("HTTP request body exceeds 4 MiB".into()),
            _ => return Err("HTTP body must be text".into()),
        };
        let mut headers = Vec::new();
        if let Some(v) = fields.get("headers") {
            let BabelData::Object(h) = v else { return Err("headers must be a Data record".into()); };
            if h.len() > 64 { return Err("Too many HTTP headers".into()); }
            for (name, value) in h {
                if name.is_empty() || name.len() > 128 || !name.bytes().all(|b|
                    b.is_ascii_alphanumeric() || b == b'-') {
                    return Err("Unsafe HTTP header name".into());
                }
                let BabelData::Text(value) = value else { return Err("HTTP header value must be text".into()); };
                if value.len() > 8192 || value.chars().any(|c| c == '\r' || c == '\n' || c == '\0') {
                    return Err("Unsafe HTTP header value".into());
                }
                headers.push(format!("{name}: {value}"));
            }
        }
        // Explicit files avoid ambiguity from binary responses containing status delimiters.
        let prefix = std::env::temp_dir().join(format!("babel-http-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos()));
        let raw_body = prefix.with_extension("body");
        let raw_headers = prefix.with_extension("headers");
        struct Clean(std::path::PathBuf, std::path::PathBuf);
        impl Drop for Clean {
            fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); let _ = std::fs::remove_file(&self.1); }
        }
        let _clean = Clean(raw_body.clone(), raw_headers.clone());
        let mut cmd = std::process::Command::new("curl");
        cmd.args(["--silent", "--show-error", "--max-time", &timeout.to_string(),
            "--max-filesize", "4194304", "--proto", "=http,https",
            "--proto-redir", "=http,https", "--request", &method,
            "--output"]).arg(&raw_body)
            .arg("--dump-header").arg(&raw_headers)
            .args(["--write-out", "%{http_code}"]);
        if redirects { cmd.args(["--location", "--max-redirs", "5"]); }
        for header in &headers { cmd.arg("--header").arg(header); }
        let has_body = fields.contains_key("body");
        if has_body {
            cmd.args(["--data-binary", "@-"]);
            cmd.stdin(std::process::Stdio::piped());
        }
        cmd.arg(url);
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = cmd.spawn().map_err(|_| "Unable to start curl".to_string())?;
        if has_body {
            use std::io::Write;
            child.stdin.take().ok_or("Missing HTTP request input")?
                .write_all(body.as_bytes()).map_err(|e| e.to_string())?;
        }
        let output = child.wait_with_output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            // Return a bounded diagnostic; curl is never invoked via a shell.
            return Err(format!("HTTP transport failed: {}",
                String::from_utf8_lossy(&output.stderr).chars().take(400).collect::<String>()));
        }
        let status = std::str::from_utf8(&output.stdout).map_err(|_| "Invalid HTTP status output")?
            .trim().parse::<i64>().map_err(|_| "Invalid HTTP status")?;
        if !(100..=599).contains(&status) { return Err("No valid HTTP response status".into()); }
        let body = std::fs::read_to_string(&raw_body).map_err(|_| "HTTP response is not UTF-8 text")?;
        if body.len() > 4 * 1024 * 1024 { return Err("HTTP response exceeds 4 MiB".into()); }
        let headers = std::fs::read_to_string(&raw_headers)
            .map_err(|e| format!("Cannot read HTTP headers: {e}"))?;
        let mut current = std::collections::BTreeMap::new();
        for line in headers.lines() {
            let line = line.trim_end_matches('\r');
            if line.starts_with("HTTP/") {
                current.clear(); // Last response headers for redirects / provisional statuses.
            } else if let Some((key, value)) = line.split_once(':') {
                let key = key.to_ascii_lowercase();
                if key.is_empty() { continue; }
                let value = value.trim().to_string();
                current.entry(key).and_modify(|v: &mut BabelData| {
                    if let BabelData::Text(old) = v {
                        old.push_str(", ");
                        old.push_str(&value);
                    }
                }).or_insert(BabelData::Text(value));
            }
        }
        let mut data = std::collections::BTreeMap::new();
        data.insert("status".into(), BabelData::Number(status.to_string()));
        data.insert("headers".into(), BabelData::Object(current));
        data.insert("body".into(), BabelData::Text(body));
        Ok(BabelData::Object(data))
    })();
    match result {
        Ok(v) => babel_result(true, v, ""),
        Err(e) => babel_result(false, BabelData::Null, &e),
    }
}
