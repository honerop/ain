// Generic AI provider primitive exposed to Ain as chat(provider, model, prompt).
// External provider tokens never enter generated source code.
use std::io::Write;
use std::process::{Command, Stdio};
fn ain_ai_json(text: &str) -> String {
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
fn ain_ai_model(model: &str) -> Result<(), String> {
    if model.len() > 128 || !model.chars().all(|c|
        c.is_ascii_alphanumeric() || matches!(c, '/'|'-'|'_'|'.'|':')
    ) { return Err("Invalid model identifier".into()); }
    Ok(())
}
fn ain_ai_deepinfra(model: &str, prompt: &str) -> Result<String, String> {
    let token = std::env::var("DEEPINFRA_API_KEY")
        .map_err(|_| "DEEPINFRA_API_KEY is not set".to_string())?;
    let selected = if model.is_empty() { "zai-org/GLM-5.3-Flash" } else { model };
    ain_ai_model(selected)?;
    let payload = format!(
        "{{\"model\":{},\"messages\":[{{\"role\":\"user\",\"content\":{}}}]}}",
        ain_ai_json(selected), ain_ai_json(prompt));
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
fn ain_ai_codex(model: &str, prompt: &str) -> Result<String, String> {
    ain_ai_model(model)?;
    let filename = std::env::temp_dir().join(format!(
        "ain-chat-{}-{}.txt", std::process::id(),
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
fn ain_chat(provider: String, model: String, prompt: String) -> String {
    let result = match provider.as_str() {
        "deepinfra" => ain_ai_deepinfra(&model, &prompt),
        "chatgpt" | "codex" => ain_ai_codex(&model, &prompt),
        _ => Err(format!("Unknown AI provider: {}", provider)),
    };
    result.unwrap_or_else(|error| format!("Provider error: {error}"))
}
