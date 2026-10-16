use ain_compiler::{compile, compile_llvm, dump_air};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::time::{SystemTime, UNIX_EPOCH};

fn json_string(s: &str) -> String {
    let mut escaped = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if c < ' ' => escaped.push_str(&format!("\\u{:04x}", c as u32)),
            c => escaped.push(c),
        }
    }
    escaped.push('"');
    escaped
}
fn usage() {
    eprintln!("AIN compiler 0.3.0\nUsage:\n  ainc check <file.ain>\n  ainc check-json <file.ain>\n  ainc air <file.ain> [-o file.air]\n  ainc llvm <file.ain> [-o file.ll]\n  ainc build-llvm <file.ain> [-o executable]\n  ainc run-llvm <file.ain>\n  ainc transpile <file.ain> [-o file.rs]\n  ainc build <file.ain> [-o executable]\n  ainc run <file.ain>\n  ainc --version\n\nThe experimental LLVM backend supports i64, bool, unit, recursion, and branching.\nOther operations still require the Rust backend.");
}
fn rustc(rust: &Path, binary: &Path) -> Result<(), String> {
    let mut cmd = Command::new("rustc");
    cmd.arg("--edition=2021").arg("-O").arg(rust).arg("-o").arg(binary);
    let generated = fs::read_to_string(rust).map_err(|e| e.to_string())?;
    if generated.contains("fn ain_gui(") {
        let output = Command::new("pkg-config")
            .args(["--libs-only-L", "gtk4"]).output()
            .map_err(|e| format!("gui needs pkg-config and GTK4: {e}"))?;
        if !output.status.success() { return Err("GTK4 is unavailable".into()); }
        for flag in String::from_utf8_lossy(&output.stdout).split_whitespace() {
            if let Some(libdir) = flag.strip_prefix("-L") {
                cmd.arg("-L").arg(format!("native={libdir}"));
            }
        }
    }
    let result = cmd.status().map_err(|e| format!("cannot run rustc: {e}"))?;
    if !result.success() { return Err(format!("rustc exited with status: {result}")); }
    Ok(())
}
fn clang(llvm: &Path, binary: &Path) -> Result<(), String> {
    let status = Command::new("clang")
        .arg("-O3").arg("-x").arg("ir")
        .arg(llvm).arg("-o").arg(binary)
        .status().map_err(|e| format!("cannot run clang: {e}; LLVM backend requires Clang"))?;
    if !status.success() { return Err(format!("clang exited with status: {status}")); }
    Ok(())
}
fn work_dir() -> Result<PathBuf, String> {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    let path = env::temp_dir().join(format!("ainc-{}-{}", process::id(), nanos));
    fs::create_dir(&path).map_err(|e| e.to_string())?;
    Ok(path)
}
fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.len() == 1 && ["--version", "-V", "version"].contains(&args[0].as_str()) {
        println!("ainc 0.3.0"); return Ok(());
    }
    if args.len() < 2 { usage(); return Err("expected command and .ain source path".into()); }
    let action = &args[0];
    let source_file = Path::new(&args[1]);
    let output = if args.len() == 4 && args[2] == "-o" { Some(PathBuf::from(&args[3])) }
        else if args.len() == 2 { None }
        else { usage(); return Err("invalid arguments".into()); };
    if !matches!(action.as_str(), "check"|"check-json"|"air"|"llvm"|"build-llvm"|"run-llvm"|"transpile"|"build"|"run") {
        usage(); return Err(format!("unknown command '{action}'"));
    }
    if output.is_some() && matches!(action.as_str(), "check"|"check-json"|"run"|"run-llvm") { return Err(format!("{action} doesn't accept -o")); }
    let source = fs::read_to_string(source_file).map_err(|e| format!("{}: {e}", source_file.display()))?;
    if action == "check-json" {
        match compile(&source) {
            Ok(c) => {
                println!("{{\"ok\":true,\"definitions\":{}}}", c.definitions);
                return Ok(());
            }
            Err(e) => {
                println!("{{\"ok\":false,\"diagnostics\":[{{\"severity\":\"error\",\"line\":{},\"message\":{}}}]}}",
                    e.line, json_string(&e.message));
                return Err("AIN source has compilation errors".into());
            }
        }
    }
    if action == "check" { let c = compile(&source).map_err(|e| e.to_string())?; println!("OK: {} definitions", c.definitions); return Ok(()); }
    if matches!(action.as_str(), "air"|"llvm"|"transpile") {
        let code = match action.as_str() {
            "air" => dump_air(&source).map_err(|e| e.to_string())?,
            "llvm" => compile_llvm(&source).map_err(|e| e.to_string())?,
            _ => compile(&source).map_err(|e| e.to_string())?.rust,
        };
        if let Some(path) = output { fs::write(&path, code).map_err(|e| e.to_string())?; println!("wrote {}", path.display()); }
        else { print!("{code}"); }
        return Ok(());
    }
    let llvm = matches!(action.as_str(), "build-llvm"|"run-llvm");
    let code = if llvm {compile_llvm(&source).map_err(|e| e.to_string())?} else {compile(&source).map_err(|e| e.to_string())?.rust};
    let tmp = work_dir()?;
    let source = tmp.join(if llvm {"program.ll"}else{"program.rs"});
    let binary = if matches!(action.as_str(), "build-llvm"|"build") {output.unwrap_or_else(|| source_file.with_extension(""))} else {tmp.join("program")};
    let result = (|| {
        fs::write(&source, code).map_err(|e| e.to_string())?;
        if llvm {clang(&source,&binary)?;} else {rustc(&source,&binary)?;}
        if matches!(action.as_str(), "build-llvm"|"build") {println!("built {}", binary.display());return Ok(());}
        let status=Command::new(&binary).status().map_err(|e| e.to_string())?;
        if !status.success() {return Err(format!("program exited with status: {status}"));}
        Ok(())
    })();
    let _ = fs::remove_dir_all(tmp);
    result
}
fn main() { if let Err(e) = run() { eprintln!("error: {e}"); process::exit(1); } }
