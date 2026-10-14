use ain_compiler::compile;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::time::{SystemTime, UNIX_EPOCH};

fn usage() {
    eprintln!("AIN compiler 0.2.0\nUsage:\n  ainc check <file.ain>\n  ainc transpile <file.ain> [-o file.rs]\n  ainc build <file.ain> [-o executable]\n  ainc run <file.ain>\n  ainc --version");
}
fn rustc(rust: &Path, binary: &Path) -> Result<(), String> {
    let result = Command::new("rustc")
        .arg("--edition=2021").arg("-O")
        .arg(rust).arg("-o").arg(binary)
        .status().map_err(|e| format!("cannot run rustc: {e}; install Rust first"))?;
    if !result.success() { return Err(format!("rustc exited with status: {result}")); }
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
        println!("ainc 0.2.0"); return Ok(());
    }
    if args.len() < 2 { usage(); return Err("expected command and .ain source path".into()); }
    let action = &args[0];
    let source_file = Path::new(&args[1]);
    let output = if args.len() == 4 && args[2] == "-o" { Some(PathBuf::from(&args[3])) }
        else if args.len() == 2 { None }
        else { usage(); return Err("invalid arguments".into()); };
    if !matches!(action.as_str(), "check"|"transpile"|"build"|"run") { usage(); return Err(format!("unknown command '{action}'")); }
    if output.is_some() && (action == "check" || action == "run") { return Err(format!("{action} doesn't accept -o")); }
    let source = fs::read_to_string(source_file).map_err(|e| format!("{}: {e}", source_file.display()))?;
    let compilation = compile(&source).map_err(|e| e.to_string())?;
    if action == "check" { println!("OK: {} definitions", compilation.definitions); return Ok(()); }
    if action == "transpile" {
        if let Some(path) = output { fs::write(&path, compilation.rust).map_err(|e| e.to_string())?; println!("wrote {}", path.display()); }
        else { print!("{}", compilation.rust); }
        return Ok(());
    }
    if action == "build" {
        let out = output.unwrap_or_else(|| source_file.with_extension(""));
        let tmp = work_dir()?;
        let rust = tmp.join("program.rs");
        let result = (|| {
            fs::write(&rust, &compilation.rust).map_err(|e| e.to_string())?;
            rustc(&rust, &out)?;
            println!("built {}", out.display());
            Ok(())
        })();
        let _ = fs::remove_dir_all(tmp);
        return result;
    }
    let tmp = work_dir()?;
    let rust = tmp.join("program.rs");
    let binary = tmp.join("program");
    let result = (|| {
        fs::write(&rust, &compilation.rust).map_err(|e| e.to_string())?;
        rustc(&rust, &binary)?;
        let status = Command::new(&binary).status().map_err(|e| e.to_string())?;
        if !status.success() { return Err(format!("program exited with status: {status}")); }
        Ok(())
    })();
    let _ = fs::remove_dir_all(tmp);
    result
}
fn main() { if let Err(e) = run() { eprintln!("error: {e}"); process::exit(1); } }
