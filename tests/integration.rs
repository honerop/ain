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

#[test]
fn external_program_no_shell() {
    assert_eq!(execute("print run \"true\" \"ignored\"\n"), "0\n");
}


#[test]
fn json_roundtrip_and_canonical_object_keys() {
    let source = r#"doc = json "{\"z\":true,\"a\":[1,2.5,null,\"a\\nb\"]}"
print encode doc
print size field doc "a"
print encode keys doc
print hasfield doc "missing"
"#;
    assert_eq!(execute(source), "{\"a\":[1,2.5,null,\"a\\nb\"],\"z\":true}\n4\n[\"a\",\"z\"]\nfalse\n");
}
#[test]
fn unicode_json_and_recoverable_parse_error() {
    let source = r#"print encode json "\"\\uD83D\\uDE00\""
print asbool field tryjson "{broken" "ok"
print astext field tryjson "{broken" "error"
"#;
    let output = execute(source);
    assert!(output.starts_with("\"😀\"\nfalse\n"));
    assert!(output.contains("Expected"));
}
#[test]
fn immutable_lists_records_pairs_and_extractors() {
    let source = r#"xs = push push array _ boxint 8 boxint 13
person = put put record _ "age" boxint 17 "name" boxstr "Ada"
print encode xs
print size xs
print asint get xs 1
print encode person
print astext field person "name"
print encode pair boxbool true nil _
"#;
    assert_eq!(execute(source), "[8,13]\n2\n13\n{\"age\":17,\"name\":\"Ada\"}\nAda\n[true,null]\n");
}
#[test]
fn functional_map_filter_fold() {
    let source = r#"double x = boxint * asint x 2
even x = == % asint x 2 0
add a b = boxint + asint a asint b
xs = push push push array _ boxint 1 boxint 2 boxint 3
print encode map "double" xs
print encode filter "even" xs
print asint fold "add" boxint 0 xs
"#;
    assert_eq!(execute(source), "[2,4,6]\n[2]\n6\n");
}
#[test]
fn function_callbacks_are_type_checked() {
    assert!(compile("f x = boxint asint x\nprint map "f" push array _ boxint 1").is_ok());
    assert!(compile("f x = asint x\nprint map "f" array _").is_err());
    assert!(compile("f a b = boxint + asint a asint b\nprint map "f" array _").is_err());
    assert!(compile("print map "missing" array _").is_err());
    assert!(compile("print asint boxstr \"wrong\"").is_err() == false); // dynamic data cast fails at runtime
}
#[test]
fn rejects_invalid_data_builtin_types() {
    assert!(compile("print push 4 2").is_err());
    assert!(compile("print json true").is_err());
    assert!(compile("print put record _ \"name\" true").is_err());
}
