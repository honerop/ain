// Standard Babel effects: files, environment, HTTP, and optional capability gating.
//
// BABEL_RESTRICTED=1 enables capability checks; allowed capabilities are
// comma-separated in BABEL_PERMISSIONS (read,write,env,process,network,
// ai,browser,desktop). Not an OS-level sandbox.
fn babel_permission_allowed(capability: &str) -> bool {
    if std::env::var("BABEL_RESTRICTED").as_deref() != Ok("1") { return true; }
    let grants = std::env::var("BABEL_PERMISSIONS").unwrap_or_default();
    grants.split(',').map(str::trim).any(|s| s == capability || s == "all")
}
fn babel_check_permission(capability: &str) {
    if !babel_permission_allowed(capability) {
        panic!("Babel capability denied: {capability}. Set BABEL_PERMISSIONS explicitly.");
    }
}
fn babel_fs_tryread(path: String) -> Result<String, String> {
    if !babel_permission_allowed("read") { return Err("Babel capability denied: read".into()); }
    std::fs::read_to_string(path).map_err(|error| error.to_string())
}
fn babel_fs_read(path: String) -> String {
    babel_check_permission("read");
    std::fs::read_to_string(path).expect("File read failed")
}
fn babel_fs_write(path: String, contents: String) {
    babel_check_permission("write");
    std::fs::write(path, contents).expect("File write failed");
}
fn babel_fs_exists(path: String) -> bool {
    babel_check_permission("read");
    std::path::Path::new(&path).exists()
}
fn babel_env_get(name: String) -> String {
    babel_check_permission("env");
    std::env::var(name).unwrap_or_default()
}
fn babel_http_request(url: String, body: Option<String>) -> String {
    babel_check_permission("network");
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
fn babel_http_get(url: String) -> String { babel_http_request(url, None) }
fn babel_http_post(url: String, body: String) -> String { babel_http_request(url, Some(body)) }
