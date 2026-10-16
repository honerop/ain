// Generic browser(op, argument) operation for Ain. Browser implementation
// is a separately sandboxed Playwright/Chromium process, not app-specific code.
use std::cell::RefCell;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
struct AinBrowser {
    process: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    driver: std::path::PathBuf,
}
impl Drop for AinBrowser {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
        let _ = std::fs::remove_file(&self.driver);
    }
}
thread_local! {
    static AIN_BROWSER: RefCell<Option<AinBrowser>> = const { RefCell::new(None) };
}
fn ain_browser_driver() -> Result<AinBrowser, String> {
    let filename = std::env::temp_dir().join(format!(
        "ain-browser-{}-{}.py", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?.as_nanos()
    ));
    std::fs::write(&filename, include_str!("ain_browser.py")).map_err(|e| e.to_string())?;
    let python = std::env::var("AIN_BROWSER_PYTHON").unwrap_or_else(|_| "python3".into());
    let mut process = Command::new(python).arg(&filename)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn().map_err(|e| format!("Playwright requires Python and the playwright package: {e}"))?;
    let input = process.stdin.take().ok_or("No browser stdin")?;
    let output = BufReader::new(process.stdout.take().ok_or("No browser stdout")?);
    Ok(AinBrowser { process, input, output, driver: filename })
}
fn ain_browser(op: String, argument: String) -> String {
    let result: Result<String, String> = AIN_BROWSER.with(|cell| {
        let mut handle = cell.borrow_mut();
        if handle.is_none() { *handle = Some(ain_browser_driver()?); }
        let browser = handle.as_mut().unwrap();
        let cmd = format!("{{\"op\":{},\"argument\":{}}}\n",
            ain_ai_json(&op), ain_ai_json(&argument));
        browser.input.write_all(cmd.as_bytes()).map_err(|e| e.to_string())?;
        browser.input.flush().map_err(|e| e.to_string())?;
        let mut line = String::new();
        if browser.output.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            return Err("Browser driver exited unexpectedly".into());
        }
        let mut jq = Command::new("jq")
            .args(["-er", "if .ok then .text else error(.error) end"])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().map_err(|e| format!("jq required for browser JSON: {e}"))?;
        jq.stdin.take().ok_or("Missing jq stdin")?
            .write_all(line.as_bytes()).map_err(|e| e.to_string())?;
        let output = jq.wait_with_output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim_end().to_string())
    });
    result.unwrap_or_else(|e| format!("Browser error: {e}"))
}
