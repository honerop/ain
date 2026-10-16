// Standard Ain effects: files, environment, HTTP, and optional capability gating.
//
// AIN_RESTRICTED=1 enables capability checks; allowed capabilities are
// comma-separated in AIN_PERMISSIONS (read,write,env,process,network,
// ai,browser,desktop). Not an OS-level sandbox.
fn ain_permission_allowed(capability: &str) -> bool {
    if std::env::var("AIN_RESTRICTED").as_deref() != Ok("1") { return true; }
    let grants = std::env::var("AIN_PERMISSIONS").unwrap_or_default();
    grants.split(',').map(str::trim).any(|s| s == capability || s == "all")
}
fn ain_check_permission(capability: &str) {
    if !ain_permission_allowed(capability) {
        panic!("Ain capability denied: {capability}. Set AIN_PERMISSIONS explicitly.");
    }
}
fn ain_fs_tryread(path: String) -> Result<String, String> {
    if !ain_permission_allowed("read") { return Err("Ain capability denied: read".into()); }
    std::fs::read_to_string(path).map_err(|error| error.to_string())
}
fn ain_fs_read(path: String) -> String {
    ain_check_permission("read");
    std::fs::read_to_string(path).expect("File read failed")
}
fn ain_fs_write(path: String, contents: String) {
    ain_check_permission("write");
    std::fs::write(path, contents).expect("File write failed");
}
fn ain_fs_exists(path: String) -> bool {
    ain_check_permission("read");
    std::path::Path::new(&path).exists()
}
fn ain_env_get(name: String) -> String {
    ain_check_permission("env");
    std::env::var(name).unwrap_or_default()
}
fn ain_http_request(url: String, body: Option<String>) -> String {
    ain_check_permission("network");
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        panic!("Only HTTP(S) URLs are supported");
    }
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["--silent", "--show-error", "--fail-with-body",
        "--max-time", "30", "--proto", "=http,https",
        "--proto-redir", "=http,https", "--location"]);
    if body.is_some() {
        cmd.args(["--request", "POST", "--header", "Content-Type: application/json",
            "--data-binary", "@-"]);
    }
    cmd.arg(url);
    if let Some(payload) = body {
        use std::io::Write;
        use std::process::Stdio;
        let mut child = cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().expect("Cannot start curl");
        child.stdin.take().expect("Missing curl stdin")
            .write_all(payload.as_bytes()).expect("HTTP request write failed");
        let output = child.wait_with_output().expect("HTTP request failed");
        assert!(output.status.success(),
            "HTTP POST failed: {}", String::from_utf8_lossy(&output.stderr));
        String::from_utf8(output.stdout).expect("HTTP response is not UTF-8")
    } else {
        let output = cmd.output().expect("Cannot start curl");
        assert!(output.status.success(),
            "HTTP GET failed: {}", String::from_utf8_lossy(&output.stderr));
        String::from_utf8(output.stdout).expect("HTTP response is not UTF-8")
    }
}
fn ain_http_get(url: String) -> String { ain_http_request(url, None) }
fn ain_http_post(url: String, body: String) -> String { ain_http_request(url, Some(body)) }
