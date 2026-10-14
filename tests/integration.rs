//! End-to-end tests. Executed by `cargo test` when rustc is available.
use ain_compiler::compile;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn execute(source: &str) -> String {
    let generated = compile(source).expect("AIN compilation").rust;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let directory: PathBuf = std::env::temp_dir().join(format!("ain-test-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let file = directory.join("main.rs");
    let binary = directory.join("run");
    fs::write(&file, generated).unwrap();
    let result = Command::new("rustc")
        .arg("--edition=2021").arg("-O").arg(&file).arg("-o").arg(&binary)
        .output().expect("rustc must be available");
    assert!(result.status.success(), "generated Rust did not compile:\n{}", String::from_utf8_lossy(&result.stderr));
    let output = Command::new(&binary).output().unwrap();
    assert!(output.status.success(), "generated binary failed:\n{}", String::from_utf8_lossy(&output.stderr));
    let _ = fs::remove_dir_all(directory);
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn arithmetic_program() {
    assert_eq!(execute(include_str!("../examples/arithmetic.ain")), "25\n");
}
#[test]
fn unit_function_program() {
    assert_eq!(execute(include_str!("../examples/hello.ain")), "I\n");
}
#[test]
fn branch_program() {
    assert_eq!(execute(include_str!("../examples/branches.ain")), "46\n6\n");
}
#[test]
fn unicode_and_strings_program() {
    assert_eq!(execute(include_str!("../examples/strings.ain")), "Hello, AGENT\n6\na_b_c\n");
}
#[test]
fn lazy_if() {
    assert_eq!(execute("print if true 42 / 1 0\n"), "42\n");
}
#[test]
fn borrowed_string_arguments() {
    assert_eq!(execute("has s = contains s \"a\"\nprint has \"cat\"\nprint has \"dog\"\n"), "true\nfalse\n");
}
#[test]
fn bool_and_unit_formatting() {
    assert_eq!(execute("print true\nprint _\nprint str 123\n"), "true\n()\n123\n");
}

#[test]
fn new_string_token_and_input_primitives() {
    let c = compile("read _ = input _\nfirst s = word s\nsecond s = skip s\nvalue = before \"abc:def\" \":\"\nprint first value\nprint isnum \"123\"\n").unwrap();
    assert!(c.rust.contains("ain_input"));
    assert!(c.rust.contains("ain_word"));
    assert!(c.rust.contains("ain_before"));
}

#[test]
fn selfhosted_bootstrap_source_typechecks_in_rust() {
    let c = compile(include_str!("../bootstrap/compiler.ain")).expect("bootstrap compiler must be valid AIN");
    assert!(c.definitions >= 40);
    assert!(c.rust.contains("fn ain_program"));
}
