// Native terminal primitives shared by all Ain CLI applications.
// argv returns a Data list of UTF-8 program arguments (excluding executable).
// readline returns a recoverable Data result: {ok, value, error}.
// Blank lines are successful; EOF is an unsuccessful result.
fn ain_argv(_: ()) -> AinData {
    AinData::List(std::env::args().skip(1).map(AinData::Text).collect())
}
fn ain_readline(_: ()) -> AinData {
    use std::io::BufRead;
    let mut line = String::new();
    match std::io::stdin().lock().read_line(&mut line) {
        Ok(0) => ain_result(false, AinData::Null, "end of input"),
        Ok(_) if line.len() > 1024 * 1024 => ain_result(false, AinData::Null, "terminal line exceeds 1 MiB"),
        Ok(_) => {
            if line.ends_with('\n') { line.pop(); }
            if line.ends_with('\r') { line.pop(); }
            ain_result(true, AinData::Text(line), "")
        }
        Err(e) => ain_result(false, AinData::Null, &e.to_string()),
    }
}
