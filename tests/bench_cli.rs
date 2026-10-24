use std::process::Command;
#[test]
fn native_benchmark_reports_measured_data() {
    let exe=env!("CARGO_BIN_EXE_babelc");
    let output=Command::new(exe).args(["bench","examples/arithmetic.babel","--backend","rust","--runs","1","--json"])
        .output().expect("babelc executable");
    assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
    let stdout=String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"compile_ms\":"),"{stdout}");
    assert!(stdout.contains("\"median_run_ms\":"),"{stdout}");
    assert!(stdout.contains("\"token_count\":null"),"{stdout}");
}
#[test]
fn benchmark_rejects_invalid_run_counts() {
    let output=Command::new(env!("CARGO_BIN_EXE_babelc"))
        .args(["bench","examples/arithmetic.babel","--runs","0"]).output().unwrap();
    assert!(!output.status.success());
}
