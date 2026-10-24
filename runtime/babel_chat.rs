// Generic AI provider primitive exposed to Babel as chat(provider, model, prompt).
// External provider tokens never enter generated source code.
use std::io::Write;
use std::process::{Command, Stdio};
fn babel_ai_json(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""), '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"), '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c <= '\u{001f}' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c)
        }
    }
    out.push('"');
    out
}
fn babel_ai_model(model: &str) -> Result<(), String> {
    if model.len() > 128 || !model.chars().all(|c|
        c.is_ascii_alphanumeric() || matches!(c, '/'|'-'|'_'|'.'|':')
    ) { return Err("Invalid model identifier".into()); }
    Ok(())
}
fn babel_ai_deepinfra(model: &str, prompt: &str) -> Result<String, String> {
    let token = std::env::var("DEEPINFRA_API_KEY")
        .map_err(|_| "DEEPINFRA_API_KEY is not set".to_string())?;
    let selected = if model.is_empty() { "zai-org/GLM-5.3-Flash" } else { model };
    babel_ai_model(selected)?;
    let payload = format!(
        "{{\"model\":{},\"messages\":[{{\"role\":\"user\",\"content\":{}}}]}}",
        babel_ai_json(selected), babel_ai_json(prompt));
    let mut curl = Command::new("curl").args([
        "--silent", "--show-error", "--fail-with-body", "--max-time", "120",
        "--request", "POST", "https://api.deepinfra.com/v1/openai/chat/completions",
        "--header", "Content-Type: application/json",
        "--header", &format!("Authorization: Bearer {token}"), "--data-binary", "@-",
    ]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().map_err(|e| format!("Cannot start curl: {e}"))?;
    curl.stdin.take().ok_or("Missing curl stdin")?
        .write_all(payload.as_bytes()).map_err(|e| e.to_string())?;
    let output = curl.wait_with_output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!("DeepInfra request failed: {}",
            String::from_utf8_lossy(&output.stderr).chars().take(500).collect::<String>()));
    }
    let mut jq = Command::new("jq")
        .args(["-er", ".choices[0].message.content | select(type == \"string\")"])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().map_err(|e| format!("Cannot start jq: {e}"))?;
    jq.stdin.take().ok_or("Missing jq stdin")?
        .write_all(&output.stdout).map_err(|e| e.to_string())?;
    let output = jq.wait_with_output().map_err(|e| e.to_string())?;
    if !output.status.success() { return Err("Unexpected DeepInfra response".into()); }
    Ok(String::from_utf8_lossy(&output.stdout).trim_end().to_string())
}
fn babel_ai_codex(model: &str, prompt: &str) -> Result<String, String> {
    babel_ai_model(model)?;
    let filename = std::env::temp_dir().join(format!(
        "babel-chat-{}-{}.txt", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?.as_nanos()
    ));
    let mut command = Command::new("codex");
    command.args(["exec", "--skip-git-repo-check", "--sandbox", "read-only",
        "--output-last-message"]).arg(&filename);
    if !model.is_empty() { command.args(["--model", model]); }
    command.arg("-").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut process = command.spawn().map_err(|e|
        format!("Cannot start official Codex CLI ({e}); run codex login first"))?;
    process.stdin.take().ok_or("Missing Codex stdin")?
        .write_all(prompt.as_bytes()).map_err(|e| e.to_string())?;
    let output = process.wait_with_output().map_err(|e| e.to_string())?;
    let result = std::fs::read_to_string(&filename);
    let _ = std::fs::remove_file(filename);
    if !output.status.success() {
        return Err(format!("Codex failed: {}",
            String::from_utf8_lossy(&output.stderr).chars().take(500).collect::<String>()));
    }
    result.map_err(|e| format!("Codex did not produce a reply: {e}"))
}
// Free-provider integration through the official OpenCode CLI. Babel programs
// contain no OpenCode-specific application language or shell scripts.
// OpenCode's free model offering may change, and may impose usage limits.
fn babel_ai_opencode(model: &str, prompt: &str) -> Result<String, String> {
    let selected = if model.is_empty() { "opencode/mimo-v2.6-flash-free" } else { model };
    babel_ai_model(selected)?;
    if prompt.len() > 128 * 1024 { return Err("OpenCode prompt exceeds 128 KiB".into()); }
    let binary = std::env::var("BABEL_OPENCODE_BIN").unwrap_or_else(|_| "opencode".into());
    let mut cmd = Command::new(binary);
    // The plan agent is the least-authority default. Workspaces and edits
    // remain explicit Ainex operations; this is NOT a sandbox guarantee.
    cmd.args(["run", "--model", selected, "--format", "json", "--agent", "plan", "--"])
        .arg(prompt);
    // Use regular files to avoid pipe-buffer deadlocks while the CLI thinks.
    let prefix = std::env::temp_dir().join(format!("babel-opencode-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos()));
    let out_path = prefix.with_extension("out");
    let err_path = prefix.with_extension("err");
    struct Cleanup(std::path::PathBuf, std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
            let _ = std::fs::remove_file(&self.1);
        }
    }
    let _clean = Cleanup(out_path.clone(),err_path.clone());
    let out = std::fs::OpenOptions::new().write(true).create_new(true)
        .open(&out_path).map_err(|e| e.to_string())?;
    let err = std::fs::OpenOptions::new().write(true).create_new(true)
        .open(&err_path).map_err(|e| e.to_string())?;
    let mut child = cmd.stdin(Stdio::null())
        .stdout(Stdio::from(out)).stderr(Stdio::from(err))
        .spawn().map_err(|e| format!("Cannot launch OpenCode CLI: {e}; install opencode"))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
    let status = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("OpenCode timed out after 180 seconds".into());
            },
            None => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    };
    let stdout = std::fs::read(&out_path).map_err(|e| e.to_string())?;
    let stderr = std::fs::read(&err_path).map_err(|e| e.to_string())?;
    if stdout.len() > 8*1024*1024 || stderr.len() > 256*1024 {
        return Err("OpenCode response exceeds allowed size".into());
    }
    let err_text = String::from_utf8_lossy(&stderr);
    if !status.success() {
        return Err(format!("OpenCode failed: {}",err_text.chars().take(600).collect::<String>()));
    }
    let lines = String::from_utf8(stdout).map_err(|_| "OpenCode JSONL is not UTF-8")?;
    let mut answer = String::new();
    let mut provider_error = None;
    for line in lines.lines().filter(|line| !line.trim().is_empty()) {
        // Some CLI versions emit status/noise outside JSON. Treat only JSON
        // event records as authoritative; malformed JSON-like events fail.
        if !line.trim_start().starts_with('{') { continue; }
        let value = babel_json_parse(line.into())
            .map_err(|e| format!("Invalid OpenCode JSON event: {e}"))?;
        let BabelData::Object(fields) = value else { continue };
        match fields.get("type") {
            Some(BabelData::Text(kind)) if kind == "text" => {
                if let Some(BabelData::Object(part)) = fields.get("part") {
                    if let Some(BabelData::Text(text)) = part.get("text") {
                        answer.push_str(text);
                    }
                }
            },
            Some(BabelData::Text(kind)) if kind == "error" => {
                provider_error = Some(line.chars().take(600).collect::<String>());
            },
            _ => {}
        }
        if answer.len() > 512*1024 {
            return Err("OpenCode answer exceeds 512 KiB".into());
        }
    }
    if let Some(error) = provider_error { return Err(format!("OpenCode response error: {error}")); }
    if answer.trim().is_empty() { return Err("OpenCode produced no text response".into()); }
    Ok(answer)
}

fn babel_chat(provider: String, model: String, prompt: String) -> String {
    let result = match provider.as_str() {
        "deepinfra" => babel_ai_deepinfra(&model, &prompt),
        "chatgpt" | "codex" => babel_ai_codex(&model, &prompt),
        "opencode" => babel_ai_opencode(&model, &prompt),
        _ => Err(format!("Unknown AI provider: {}", provider)),
    };
    result.unwrap_or_else(|error| format!("Provider error: {error}"))
}
