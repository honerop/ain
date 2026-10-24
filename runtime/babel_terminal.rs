// Native terminal primitives shared by all Babel CLI applications.
// argv returns a Data list of UTF-8 program arguments (excluding executable).
// readline returns a recoverable Data result: {ok, value, error}.
// Blank lines are successful; EOF is an unsuccessful result.
fn babel_argv(_: ()) -> BabelData {
    BabelData::List(std::env::args().skip(1).map(BabelData::Text).collect())
}
fn babel_readline(_: ()) -> BabelData {
    use std::io::BufRead;
    let mut line = String::new();
    match std::io::stdin().lock().read_line(&mut line) {
        Ok(0) => babel_result(false, BabelData::Null, "end of input"),
        Ok(_) if line.len() > 1024 * 1024 => babel_result(false, BabelData::Null, "terminal line exceeds 1 MiB"),
        Ok(_) => {
            if line.ends_with('\n') { line.pop(); }
            if line.ends_with('\r') { line.pop(); }
            babel_result(true, BabelData::Text(line), "")
        }
        Err(e) => babel_result(false, BabelData::Null, &e.to_string()),
    }
}
