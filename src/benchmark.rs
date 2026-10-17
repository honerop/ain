//! Native Ain compiler benchmark, independent of Python or model providers.
//! Executes the supplied program. Never benchmark untrusted programs without OS isolation.
use super::{clang, compile, compile_llvm, json_string, rustc, work_dir};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

#[derive(Clone)]
struct Sample {
    kind: &'static str,
    compile_ms: f64,
    median_run_ms: f64,
    binary_bytes: u64,
    stdout: String,
    codes: Vec<i32>,
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn median(mut durations: Vec<f64>) -> f64 {
    durations.sort_by(|a,b| a.total_cmp(b));
    let len=durations.len();
    if len % 2 == 0 { (durations[len/2-1]+durations[len/2])/2.0 }
    else { durations[len/2] }
}

fn capture(mut child: Child, stdout: &Path, stderr: &Path, timeout: Duration) -> Result<(i32,String),String> {
    let started=Instant::now();
    let status=loop {
        if let Some(status)=child.try_wait().map_err(|e|e.to_string())? { break status; }
        if started.elapsed()>=timeout {
            let _=child.kill(); let _=child.wait();
            return Err(format!("benchmarked program exceeded {} ms",timeout.as_millis()));
        }
        sleep(Duration::from_millis(5));
    };
    for file in [stdout,stderr] {
        if fs::metadata(file).map_err(|e|e.to_string())?.len()>8*1024*1024 {
            return Err("benchmark output exceeded 8 MiB".into());
        }
    }
    let output=String::from_utf8(fs::read(stdout).map_err(|e|e.to_string())?)
        .map_err(|_|"benchmark stdout was not UTF-8")?;
    if !status.success() {
        let error=String::from_utf8_lossy(&fs::read(stderr).map_err(|e|e.to_string())?).to_string();
        return Err(format!("benchmark program exited with {}: {}",status,error.chars().take(500).collect::<String>()));
    }
    Ok((status.code().unwrap_or(-1),output))
}

fn measure(source: &str, kind: &'static str, repeats: usize, timeout: Duration, tmp: &Path) -> Result<Sample,String> {
    let started=Instant::now();
    let generated=match kind {
        "rust"=>compile(source).map_err(|e|e.to_string())?.rust,
        "llvm"=>compile_llvm(source).map_err(|e|e.to_string())?,
        _=>return Err("unknown backend".into()),
    };
    let source_file=tmp.join(format!("bench-{kind}.{}",if kind=="rust" {"rs"}else{"ll"}));
    let binary=tmp.join(format!("bench-{kind}{}",env::consts::EXE_SUFFIX));
    fs::write(&source_file,generated).map_err(|e|e.to_string())?;
    if kind=="rust" {rustc(&source_file,&binary)?;} else {clang(&source_file,&binary)?;}
    let compile_ms=started.elapsed().as_secs_f64()*1000.0;
    let binary_bytes=fs::metadata(&binary).map_err(|e|e.to_string())?.len();
    let mut durations=Vec::with_capacity(repeats);
    let mut output: Option<String>=None;
    let mut codes=Vec::with_capacity(repeats);
    for idx in 0..repeats {
        let stdout_file=tmp.join(format!("{kind}-{idx}.stdout"));
        let stderr_file=tmp.join(format!("{kind}-{idx}.stderr"));
        let stdout=fs::File::create(&stdout_file).map_err(|e|e.to_string())?;
        let stderr=fs::File::create(&stderr_file).map_err(|e|e.to_string())?;
        let run_start=Instant::now();
        let child=Command::new(&binary).stdin(Stdio::null())
            .stdout(Stdio::from(stdout)).stderr(Stdio::from(stderr))
            .spawn().map_err(|e|e.to_string())?;
        let (code,observed)=capture(child,&stdout_file,&stderr_file,timeout)?;
        durations.push(run_start.elapsed().as_secs_f64()*1000.0);
        codes.push(code);
        if let Some(previous)=&output {
            if previous!=&observed { return Err(format!("{kind} output changed between runs")); }
        } else {output=Some(observed);}
    }
    Ok(Sample {kind,compile_ms,median_run_ms:median(durations),binary_bytes,
        stdout:output.unwrap_or_default(),codes})
}

pub fn run(args: &[String]) -> Result<(),String> {
    let Some(file) = args.first() else {return Err("usage: ainc bench FILE.ain [--backend rust|llvm|both] [--runs N] [--timeout-ms N] [--json]".into())};
    let mut backend="rust";
    let mut repeats=3usize;
    let mut timeout_ms=5000u64;
    let mut json=false;
    let mut i=1;
    while i<args.len() {
        match args[i].as_str() {
            "--backend" => {i+=1;backend=args.get(i).ok_or("--backend requires a value")?;if !["rust","llvm","both"].contains(&backend) {return Err("invalid backend".into());}},
            "--runs" => {i+=1;repeats=args.get(i).ok_or("--runs requires a value")?.parse().map_err(|_|"invalid run count")?;if !(1..=100).contains(&repeats) {return Err("runs must be between 1 and 100".into());}},
            "--timeout-ms" => {i+=1;timeout_ms=args.get(i).ok_or("--timeout-ms requires a value")?.parse().map_err(|_|"invalid timeout")?;if !(1..=300000).contains(&timeout_ms) {return Err("timeout-ms must be between 1 and 300000".into());}},
            "--json" => json=true,
            other => return Err(format!("unknown benchmark flag '{other}'")),
        }
        i+=1;
    }
    let source=fs::read_to_string(file).map_err(|e|format!("{file}: {e}"))?;
    let tmp=Temporary(work_dir()?);
    let mut measurements=Vec::new();
    for kind in match backend { "both" => vec!["rust","llvm"], "rust" => vec!["rust"], "llvm" => vec!["llvm"], _ => unreachable!() } {
        measurements.push(measure(&source,kind,repeats,Duration::from_millis(timeout_ms),&tmp.0)?);
    }
    let equivalent=if measurements.len()==2 {
        measurements[0].stdout==measurements[1].stdout && measurements[0].codes==measurements[1].codes
    } else {true};
    if json {
        print!("{{\"schema\":1,\"source_bytes\":{},\"runs\":{},\"token_count\":null,\"equivalent\":{},\"backends\":[",
            source.len(),repeats,equivalent);
        for (idx,m) in measurements.iter().enumerate() {
            if idx>0 {print!(",");}
            print!("{{\"backend\":{},\"compile_ms\":{:.3},\"median_run_ms\":{:.3},\"binary_bytes\":{},\"stdout\":{},\"exit_codes\":[",
                json_string(m.kind),m.compile_ms,m.median_run_ms,m.binary_bytes,json_string(&m.stdout));
            for (j,code) in m.codes.iter().enumerate(){if j>0 {print!(",");}print!("{code}");}
            print!("]}}");
        }
        println!("]}}");
    } else {
        println!("Ain benchmark: {file} ({repeats} runs)");
        for m in &measurements {
            println!("  {}: compile {:.2} ms, median run {:.2} ms, executable {} bytes",
                m.kind,m.compile_ms,m.median_run_ms,m.binary_bytes);
        }
        if backend=="both" {println!("  Rust / LLVM output & exit codes: {}",if equivalent {"match"} else {"MISMATCH"});}
        println!("  LLM token usage: not measured");
    }
    if !equivalent {return Err("Rust and LLVM benchmark results differ".into());}
    Ok(())
}
