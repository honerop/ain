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
    assert!(compile(r#"f x = boxint asint x
print map "f" push array _ boxint 1"#).is_ok());
    assert!(compile(r#"f x = asint x
print map "f" array _"#).is_err());
    assert!(compile(r#"f a b = boxint + asint a asint b
print map "f" array _"#).is_err());
    assert!(compile(r#"print map "missing" array _"#).is_err());
    assert!(compile(r#"print map add array _"#).is_err());
    assert!(compile(r#"print asint boxstr "wrong""#).is_ok());
}
#[test]
fn rejects_invalid_data_builtin_types() {
    assert!(compile("print push 4 2").is_err());
    assert!(compile("print json true").is_err());
    assert!(compile("print put record _ \"name\" true").is_err());
}


#[test]
fn functional_iteration_and_explicit_mutable_cells() {
    let source = r#"advance x = boxint + asint x 1
show x = print asint x
numbers = push push array _ boxint 3 boxint 4
print asint iterate "advance" 5 boxint 10
each "show" numbers
store = cell boxint 7
print asint readstate store
print asint swapstate store boxint 9
print asint readstate store
setstate store boxint 11
print asint readstate store
"#;
    assert_eq!(execute(source), "15\n3\n4\n7\n7\n9\n11\n");
}
#[test]
fn loop_and_state_type_errors() {
    assert!(compile(r#"f x = boxint + asint x 1
print iterate "f" false boxint 0"#).is_err());
    assert!(compile("print readstate boxint 3").is_err());
    assert!(compile("print setstate boxint 3 boxint 2").is_err());
}


#[test]
fn structured_compiler_diagnostics() {
    let dir = std::env::temp_dir().join(format!("ain-diagnostics-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bad.ain");
    fs::write(&file, "print + true 3\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ainc"))
        .arg("check-json").arg(&file).output().unwrap();
    assert!(!output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains(r#""ok":false"#));
    assert!(text.contains(r#""line":1"#));
    assert!(text.contains("type mismatch"));
    fs::write(&file, "print + 2 3\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ainc"))
        .arg("check-json").arg(&file).output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout).unwrap().contains(r#""ok":true"#));
    let _ = fs::remove_dir_all(dir);
}


#[test]
fn async_task_completes_and_can_be_polled() {
    let source = r#"double x = boxint * asint x 2
task = spawn "double" boxint 21
result = wait task
print asint result
print asbool field poll task "ok"
"#;
    assert_eq!(execute(source), "42\ntrue\n");
}
#[test]
fn async_callback_types_are_checked() {
    assert!(compile(r#"f x = asint x
task = spawn "f" boxint 1"#).is_err());
    assert!(compile("print wait boxint 5").is_err());
    assert!(compile(r#"task = spawn "missing" boxint 1"#).is_err());
}
#[test]
fn file_read_write_and_exists() {
    let path = std::env::temp_dir().join(format!("ain-fs-{}.txt", std::process::id()));
    let source = format!(
        "fswrite {:?} \"Hello Ain\"\\nprint fsread {:?}\\nprint fsexists {:?}\\n",
        path.to_str().unwrap(), path.to_str().unwrap(), path.to_str().unwrap()
    ).replace("\\n", "\n");
    assert_eq!(execute(&source), "Hello Ain\ntrue\n");
    fs::remove_file(&path).unwrap();
}
#[test]
fn native_http_get_local_server() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut buf = [0u8; 4096];
        let n = stream.read(&mut buf).unwrap();
        assert!(String::from_utf8_lossy(&buf[..n]).starts_with("GET /ping"));
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 4\r\nConnection: close\r\n\r\npong").unwrap();
    });
    let source = format!("print httpget \"http://127.0.0.1:{port}/ping\"\n");
    assert_eq!(execute(&source), "pong\n");
    worker.join().unwrap();
}
#[test]
fn permission_checks_reject_unauthorized_file_writes() {
    let source = "fswrite \"/tmp/ain-permission-probe\" \"secret\"\n";
    let compiled = compile(source).unwrap().rust;
    let dir = std::env::temp_dir().join(format!("ain-permission-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let rs = dir.join("main.rs");
    let exe = dir.join("probe");
    fs::write(&rs, compiled).unwrap();
    let out = Command::new("rustc").args(["--edition=2021", "-O"])
        .arg(&rs).arg("-o").arg(&exe).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let denied = Command::new(&exe).env("AIN_RESTRICTED", "1")
        .env_remove("AIN_PERMISSIONS").output().unwrap();
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr).contains("capability denied"));
    let _ = fs::remove_dir_all(dir);
}
#[test]
fn effect_builtin_type_errors() {
    assert!(compile("print fsread 7").is_err());
    assert!(compile("fswrite \"file\" 7").is_err());
    assert!(compile("print httpget true").is_err());
}


#[test]
fn sqlite_parameterized_queries_and_persistence() {
    let path = std::env::temp_dir().join(format!(
        "ain-sqlite-{}-{}.db", std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    ));
    let p = path.to_string_lossy();
    let source = format!(r#"path = {p:?}
empty = array _
created = sqlexec path "CREATE TABLE examples (name TEXT, score INTEGER)" empty
print asbool field created "ok"
params = push push array _ boxstr "A'); DROP TABLE examples; --" boxint 17
added = sqlexec path "INSERT INTO examples(name, score) VALUES(?1, ?2)" params
print asbool field added "ok"
results = sqlquery path "SELECT name, score FROM examples WHERE score = ?" push array _ boxint 17
print encode field results "value"
print asbool field results "ok"
retry = sqlquery path "SELECT name FROM examples" empty
print size field retry "value"
error = sqlquery path "SELECT missing FROM examples" empty
print asbool field error "ok"
print hasfield error "error"
"#);
    let output = execute(&source);
    assert_eq!(output,
        "true\ntrue\n[{\"name\":\"A'); DROP TABLE examples; --\",\"score\":17}]\ntrue\n1\nfalse\ntrue\n");
    let _ = fs::remove_file(path);
}
#[test]
fn sqlite_parameter_count_and_multiple_statement_rejection() {
    let path = std::env::temp_dir().join(format!(
        "ain-sqlite-reject-{}.db", std::process::id()
    ));
    let p = path.to_string_lossy();
    let source = format!(r#"db = {p:?}
err1 = sqlexec db "SELECT ?" array _
err2 = sqlexec db "SELECT 1; SELECT 2" array _
print asbool field err1 "ok"
print asbool field err2 "ok"
"#);
    assert_eq!(execute(&source), "false\nfalse\n");
    let _ = fs::remove_file(path);
}
#[test]
fn sqlite_type_checks() {
    assert!(compile("print sqlquery 4 \"select 1\" array _").is_err());
    assert!(compile("print sqlexec \"a.db\" \"select 1\" true").is_err());
    assert!(compile("print sqlquery \"a.db\" \"select 1\" array _").is_ok());
}


#[test]
fn csv_quoted_records_unicode_and_roundtrip() {
    let source = r#"rows = csvparse "name,quote\r\nAda,\"Hello, \"\"world\"\"\"\r\nLéa,\"two\nlines\""
print encode rows
print csvwrite rows
"#;
    assert_eq!(execute(source),
        "[[\"name\",\"quote\"],[\"Ada\",\"Hello, \\\"world\\\"\"],[\"Léa\",\"two\\nlines\"]]\nname,quote\r\nAda,\"Hello, \"\"world\"\"\"\r\nLéa,\"two\nlines\"\n");
}
#[test]
fn csv_recoverable_malformed_input() {
    let source = r#"bad = trycsv "name,\"unterminated"
print asbool field bad "ok"
print astext field bad "error"
"#;
    assert_eq!(execute(source), "false\nUnterminated quoted CSV field\n");
}
#[test]
fn base64_utf8_bytes_and_corruption() {
    let source = r#"payload = utf8bytes "Hello 😀"
encoded = b64encode payload
print encoded
decoded = b64decode encoded
text = frombytes field decoded "value"
print astext field text "value"
invalid = b64decode "Zm9="
print asbool field invalid "ok"
"#;
    assert_eq!(execute(source), "SGVsbG8g8J+YgA==\nHello 😀\nfalse\n");
}
#[test]
fn binary_deflate_roundtrip_and_rejects_corruption() {
    let source = r#"raw = utf8bytes "Ain data compresses into bytes"
packed = zpack raw
result = zunpack packed
text = frombytes field result "value"
print asbool field result "ok"
print astext field text "value"
invalid = zunpack utf8bytes "invalid"
print asbool field invalid "ok"
"#;
    assert_eq!(execute(source), "true\nAin data compresses into bytes\nfalse\n");
}
#[test]
fn serialization_type_checks() {
    assert!(compile("print csvparse 42").is_err());
    assert!(compile("print csvwrite true").is_err());
    assert!(compile("print zpack false").is_err());
    assert!(compile("print frombytes boxstr \"not a list\"").is_ok());
}

#[test]
fn typed_list_construction_and_access() {
    let source = r#"xs = listpush listpush listof 1 2 3
print xs
print listlen xs
print listat xs 1
names = listpush listof "Ada" "Léa"
print listat names 0
nested = listof listof 9
print nested
"#;
    assert_eq!(execute(source), "[1,2,3]\n3\n2\nAda\n[[9]]\n");
}
#[test]
fn typed_functional_list_operations() {
    let source = r#"double x = * x 2
even x = == % x 2 0
add x y = + x y
shout x = upper x
xs = listpush listpush listof 1 2 3
print listmap "double" xs
print listfilter "even" xs
print listfold "add" 10 xs
words = listof "hello"
print listmap "shout" words
"#;
    assert_eq!(execute(source), "[2,4,6]\n[2]\n16\n[HELLO]\n");
}
#[test]
fn typed_lists_reject_mixed_elements_or_wrong_callbacks() {
    assert!(compile("print listpush listof 1 true").is_err());
    assert!(compile("print listat listof 1 true").is_err());
    assert!(compile("print listlen true").is_err());
    assert!(compile("print listmap \"missing\" listof 1").is_err());
    assert!(compile("bad x = + x 1\nprint listfilter \"bad\" listof 1").is_err());
}

#[test]
fn typed_options_are_explicit_and_typechecked() {
    let source = r#"known = option_some 42
missing = option_none _
print option_has known
print option_get known 3
print option_get missing 9
print option_has option_some "hello"
"#;
    assert_eq!(execute(source), "true\n42\n9\ntrue\n");
    assert!(compile("print option_get option_some 2 \"wrong\"").is_err());
}
#[test]
fn typed_result_values_are_recoverable() {
    let source = r#"good = result_ok 7
failed = result_err "offline"
print result_valid good
print result_get good 1
print result_get failed 10
print result_error failed
"#;
    assert_eq!(execute(source), "true\n7\n10\noffline\n");
    assert!(compile("print result_get result_ok true 0").is_err());
}
#[test]
fn tryread_handles_missing_files() {
    let file = format!("/tmp/ain-nonexistent-{}-test", std::process::id());
    let source = format!(r#"result = tryread {file:?}
print result_valid result
print result_get result "fallback"
"#);
    assert_eq!(execute(&source), "false\nfallback\n");
}

#[test]
fn native_gui_event_pump_is_type_checked() {
    assert!(compile(include_str!("../examples/gui_tick.ain")).is_ok());
    assert!(compile("guiupdate 4 \"layout\"").is_err());
    assert!(compile("print guitick \"forty\"").is_err());
}
#[test]
fn native_gui_event_pump_is_linked_only_on_demand() {
    let code = compile(include_str!("../examples/gui_tick.ain")).unwrap().rust;
    assert!(code.contains("fn ain_guiupdate("));
    assert!(code.contains("fn ain_guitick("));
    let unrelated = compile("print + 1 1").unwrap().rust;
    assert!(!unrelated.contains("fn ain_guiupdate("));
}

#[test]
fn structured_http_response_status_headers_and_request_body() {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(4))).unwrap();
        let mut received = [0; 8192];
        let n = stream.read(&mut received).unwrap();
        let request = String::from_utf8_lossy(&received[..n]);
        assert!(request.starts_with("POST /echo HTTP/1.1"), "{request}");
        assert!(request.contains("X-Token: secret"), "{request}");
        assert!(request.contains("Content-Length: 5"), "{request}");
        stream.write_all(b"HTTP/1.1 201 Created\r\nX-Response: success\r\nContent-Type: text/plain\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok").unwrap();
    });
    let source = format!(r#"headers = put record _ "X-Token" boxstr "secret"
options = put put record _ "headers" headers "body" boxstr "hello"
response = httprequest "POST" "http://127.0.0.1:{port}/echo" options
print asbool field response "ok"
details = field response "value"
print asint field details "status"
print astext field field details "headers" "x-response"
print astext field details "body"
"#);
    assert_eq!(execute(&source), "true\n201\nsuccess\nok\n");
    server.join().unwrap();
}
#[test]
fn structured_http_options_and_header_injection_are_rejected() {
    let source = r#"options = put record _ "headers" put record _ "X-Unsafe" boxstr "a\nInjected: b"
a = httprequest "GET" "https://example.com" options
print asbool field a "ok"
print astext field a "error"
b = httprequest "TRACE" "https://example.com" record _
print asbool field b "ok"
c = httprequest "GET" "file:///etc/passwd" record _
print asbool field c "ok"
"#;
    let result = execute(source);
    assert!(result.contains("false\nUnsafe HTTP header value\nfalse\nfalse\n"), "{result}");
}
#[test]
fn structured_http_types_are_checked() {
    assert!(compile("print httprequest \"GET\" \"https://example.com\" record _").is_ok());
    assert!(compile("print httprequest 10 \"https://example.com\" record _").is_err());
    assert!(compile("print httprequest \"GET\" 9 record _").is_err());
    assert!(compile("print httprequest \"GET\" \"https://example.com\" false").is_err());
}

#[test]
fn typed_maps_have_inferred_value_types_and_stable_keys() {
    let source = r#"scores = mapset mapone "z" 9 "a" 3
print scores
print maplen scores
print mapat scores "a" 0
print mapat scores "missing" 42
print maphas scores "z"
print mapkeys scores
print mapvalues scores
print mapremove scores "z"
words = mapset mapone "one" "alpha" "two" "beta"
print mapat words "two" "missing"
nested = mapone "children" listof 7
print listat mapat nested "children" listof 0 0
"#;
    assert_eq!(execute(source), "{a:3,z:9}\n2\n3\n42\ntrue\n[a,z]\n[3,9]\n{a:3}\nbeta\n7\n");
}
#[test]
fn typed_maps_reject_wrong_key_value_and_fallback_types() {
    assert!(compile("print mapone 1 2").is_err());
    assert!(compile("print mapset mapone \"a\" 2 \"b\" true").is_err());
    assert!(compile("print mapat mapone \"a\" 2 \"a\" \"wrong\"").is_err());
    assert!(compile("print maplen 3").is_err());
    assert!(compile("print maphas mapone \"a\" 2 1").is_err());
}
