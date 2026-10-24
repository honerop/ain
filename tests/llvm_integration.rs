use babel_compiler::{compile_llvm, dump_air};
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::atomic::{AtomicU64, Ordering};
static TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn clang_available() -> bool {
    Command::new("clang").arg("--version").output().is_ok()
}
fn execute(source: &str) -> (bool, String) {
    let llvm = compile_llvm(source).expect("AIR lowering");
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("babel-llvm-test-{}-{stamp}-{}", std::process::id(), TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir(&dir).unwrap();
    let ir = dir.join("program.ll");
    let binary = dir.join(format!("program{}",std::env::consts::EXE_SUFFIX));
    fs::write(&ir, llvm).unwrap();
    let compilation = Command::new("clang").arg("-O3").arg("-x").arg("ir")
        .arg(&ir).arg("-o").arg(&binary).output().expect("clang available");
    assert!(compilation.status.success(), "clang rejected LLVM IR:\n{}", String::from_utf8_lossy(&compilation.stderr));
    let result = Command::new(&binary).output().unwrap();
    let ok = result.status.success();
    let text = String::from_utf8(result.stdout).unwrap();
    let _ = fs::remove_dir_all(&dir);
    (ok, text)
}
#[test]
fn typed_ir_exposes_concrete_types() {
    let ir = dump_air("square x = * x x\nprint square 8\n").unwrap();
    assert!(ir.contains("func square x:i64 -> i64"));
    assert!(ir.contains("eval"));
}
#[test]
fn llvm_uses_unboxed_values() {
    let llvm = compile_llvm("square x = * x x\nprint square 8\n").unwrap();
    assert!(llvm.contains("define i64 @babel_square(i64 %arg0)"));
    assert!(!llvm.contains("%struct.V"));
    assert!(!llvm.contains("@v_mul"));
}
#[test]
fn llvm_supports_examples() {
    if !clang_available() { eprintln!("skipping: clang not installed"); return; }
    for (program,expected) in [
        (include_str!("../examples/arithmetic.babel"),"25\n"),
        (include_str!("../examples/branches.babel"),"46\n6\n"),
        (include_str!("../examples/recursive.babel"),"720\n42\n"),
    ] {
        let (success, out) = execute(program);
        assert!(success);
        assert_eq!(out, expected);
    }
}
#[test]
fn llvm_respects_short_circuit() {
    if !clang_available() { return; }
    let (ok, out) = execute("print if true 42 / 1 0\nprint and false > / 1 0 0\nprint or true > / 1 0 0\n");
    assert!(ok);
    assert_eq!(out, "42\nfalse\ntrue\n");
}
#[test]
fn llvm_preserves_integer_guards() {
    if !clang_available() { return; }
    let (ok, _) = execute("x = + 9223372036854775807 1\nprint x\n");
    assert!(!ok, "overflow must terminate rather than silently wrap");
}
#[test]
fn llvm_rejects_polymorphic_functions_gracefully() {
    let err=compile_llvm("id x = x\nprint id 1\n").unwrap_err();
    assert!(err.message.contains("polymorphic"),"{}",err.message);
}
#[test]
fn llvm_rejects_strings_until_supported() {
    assert!(compile_llvm("print \"hello\"").is_err());
}
#[test]
fn llvm_unit_functions() {
    if !clang_available() { return; }
    let (ok,out)=execute("twice _ = print 7\ntwice _\ntwice _\n");
    assert!(ok);
    assert_eq!(out,"7\n7\n");
}
#[test]
fn llvm_assertion_is_checked() {
    if !clang_available() { return; }
    let (ok, output) = execute("assert == + 2 3 5\nprint 17\n");
    assert!(ok);
    assert_eq!(output, "17\n");
    let (ok, _) = execute("assert > 1 3\n");
    assert!(!ok, "failed assertion must trap in LLVM backend");
}

#[test]
fn llvm_constant_folding() {
    let air=dump_air("x = + 4 8\nprint x\n").unwrap();
    assert!(air.contains("let x:i64 = 12:i64"));
}

#[test]
fn llvm_min_max_pow_and_clamp() {
    if !clang_available() { return; }
    let (ok,out)=execute("print pow 2 10\nprint min 5 9\nprint max 5 9\nprint clamp 15 0 10\n");
    assert!(ok);
    assert_eq!(out,"1024\n5\n9\n10\n");
}
#[test]
fn llvm_runtime_division_by_zero_traps() {
    if !clang_available() { return; }
    let (ok,_) = execute("x = / 99 0\nprint x\n");
    assert!(!ok);
}
