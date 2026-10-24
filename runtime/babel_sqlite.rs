// SQLite bindings for Babel's Rust backend. Linked only for programs using sqlquery/sqlexec.
// Statements are prepared and bound: values never interpolate into SQL text.
// Each call owns a connection, query handles stay contained inside the call.
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::ptr;
#[link(name="sqlite3")]
unsafe extern "C" {
    fn sqlite3_open_v2(path: *const c_char, db: *mut *mut c_void,
        flags: c_int, vfs: *const c_char) -> c_int;
    fn sqlite3_close(db: *mut c_void) -> c_int;
    fn sqlite3_errmsg(db: *mut c_void) -> *const c_char;
    fn sqlite3_prepare_v2(db: *mut c_void, sql: *const c_char, len: c_int,
        statement: *mut *mut c_void, tail: *mut *const c_char) -> c_int;
    fn sqlite3_finalize(statement: *mut c_void) -> c_int;
    fn sqlite3_bind_parameter_count(statement: *mut c_void) -> c_int;
    fn sqlite3_bind_null(statement: *mut c_void, position: c_int) -> c_int;
    fn sqlite3_bind_int64(statement: *mut c_void, position: c_int, value: i64) -> c_int;
    fn sqlite3_bind_double(statement: *mut c_void, position: c_int, value: f64) -> c_int;
    fn sqlite3_bind_text(statement: *mut c_void, position: c_int, value: *const c_char,
        len: c_int, destroy: Option<unsafe extern "C" fn(*mut c_void)>) -> c_int;
    fn sqlite3_column_count(statement: *mut c_void) -> c_int;
    fn sqlite3_column_name(statement: *mut c_void, col: c_int) -> *const c_char;
    fn sqlite3_column_type(statement: *mut c_void, col: c_int) -> c_int;
    fn sqlite3_column_text(statement: *mut c_void, col: c_int) -> *const u8;
    fn sqlite3_column_int64(statement: *mut c_void, col: c_int) -> i64;
    fn sqlite3_column_double(statement: *mut c_void, col: c_int) -> f64;
    fn sqlite3_step(statement: *mut c_void) -> c_int;
    fn sqlite3_changes(db: *mut c_void) -> c_int;
}
struct BabelDb(*mut c_void);
impl Drop for BabelDb { fn drop(&mut self) { unsafe { sqlite3_close(self.0); } } }
struct BabelStatement(*mut c_void);
impl Drop for BabelStatement { fn drop(&mut self) { unsafe { sqlite3_finalize(self.0); } } }
fn babel_sql_err(db: *mut c_void) -> String {
    unsafe { CStr::from_ptr(sqlite3_errmsg(db)).to_string_lossy().into_owned() }
}
fn babel_sql_run(path: String, sql: String, params: BabelData, query: bool) -> BabelData {
    let result = (|| -> Result<BabelData, String> {
        babel_check_permission("database");
        let path = CString::new(path).map_err(|_| "Database path contains NUL")?;
        let sql = CString::new(sql).map_err(|_| "SQL contains NUL")?;
        let BabelData::List(parameters) = params else {
            return Err("SQL parameters must be a Data list".into());
        };
        let mut raw = ptr::null_mut();
        let code = unsafe { sqlite3_open_v2(path.as_ptr(), &mut raw,
            0x00000002 | 0x00000004 | 0x00010000, ptr::null()) };
        if raw.is_null() { return Err("Could not open SQLite database".into()); }
        let db = BabelDb(raw);
        if code != 0 { return Err(babel_sql_err(db.0)); }
        let mut statement = ptr::null_mut();
        let mut tail = ptr::null();
        if unsafe { sqlite3_prepare_v2(db.0, sql.as_ptr(), -1, &mut statement, &mut tail) } != 0 {
            return Err(babel_sql_err(db.0));
        }
        if statement.is_null() { return Err("Empty SQL statement".into()); }
        let statement = BabelStatement(statement);
        if !unsafe { CStr::from_ptr(tail) }.to_string_lossy().trim().is_empty() {
            return Err("Only one SQL statement per call is allowed".into());
        }
        let count = unsafe { sqlite3_bind_parameter_count(statement.0) };
        if count as usize != parameters.len() {
            return Err(format!("Expected {count} parameters, got {}", parameters.len()));
        }
        let mut bound_strings = Vec::<CString>::new();
        for (i, parameter) in parameters.iter().enumerate() {
            let pos = (i + 1) as i32;
            let bind = unsafe { match parameter {
                BabelData::Null => sqlite3_bind_null(statement.0, pos),
                BabelData::Bool(b) => sqlite3_bind_int64(statement.0, pos, if *b { 1 } else { 0 }),
                BabelData::Number(n) => {
                    if let Ok(integer) = n.parse::<i64>() {
                        sqlite3_bind_int64(statement.0, pos, integer)
                    } else {
                        let number = n.parse::<f64>().map_err(|_| "Invalid numeric SQL parameter")?;
                        if !number.is_finite() { return Err("SQL number is not finite".into()); }
                        sqlite3_bind_double(statement.0, pos, number)
                    }
                }
                value => {
                    let text = match value { BabelData::Text(v) => v.clone(), _ => value.json() };
                    bound_strings.push(CString::new(text).map_err(|_| "SQL text contains NUL")?);
                    sqlite3_bind_text(statement.0, pos, bound_strings.last().unwrap().as_ptr(), -1, None)
                }
            }};
            if bind != 0 { return Err(babel_sql_err(db.0)); }
        }
        // Named-column objects; duplicate column names are rejected to avoid silent data loss.
        let cols = unsafe { sqlite3_column_count(statement.0) };
        let mut names = Vec::new();
        if query {
            for i in 0..cols {
                let name = unsafe { CStr::from_ptr(sqlite3_column_name(statement.0, i)) }
                    .to_string_lossy().into_owned();
                if names.contains(&name) { return Err("Duplicate result column name".into()); }
                names.push(name);
            }
        }
        let mut rows = Vec::new();
        loop {
            match unsafe { sqlite3_step(statement.0) } {
                101 => break, // SQLITE_DONE
                100 if query => {
                    if rows.len() >= 10_000 { return Err("Query returned too many rows".into()); }
                    let mut item = std::collections::BTreeMap::new();
                    for (i, name) in names.iter().enumerate() {
                        let col = i as i32;
                        let value = unsafe { match sqlite3_column_type(statement.0, col) {
                            1 => BabelData::Number(sqlite3_column_int64(statement.0, col).to_string()),
                            2 => {
                                let value = sqlite3_column_double(statement.0, col);
                                if !value.is_finite() { BabelData::Null }
                                else { BabelData::Number(value.to_string()) }
                            }
                            3 => {
                                let ptr = sqlite3_column_text(statement.0, col);
                                if ptr.is_null() { BabelData::Text(String::new()) }
                                else { BabelData::Text(CStr::from_ptr(ptr as *const c_char)
                                    .to_string_lossy().into_owned()) }
                            },
                            4 => return Err("SQLite BLOB columns are not yet supported".into()),
                            _ => BabelData::Null,
                        }};
                        item.insert(name.clone(), value);
                    }
                    rows.push(BabelData::Object(item));
                }
                100 => {}, // Execute statement discards returned rows.
                _ => return Err(babel_sql_err(db.0)),
            }
        }
        Ok(if query { BabelData::List(rows) }
            else { BabelData::Number(unsafe { sqlite3_changes(db.0) }.to_string()) })
    })();
    match result {
        Ok(value) => babel_result(true, value, ""),
        Err(error) => babel_result(false, BabelData::Null, &error)
    }
}
fn babel_sqlquery(path: String, statement: String, params: BabelData) -> BabelData {
    babel_sql_run(path, statement, params, true)
}
fn babel_sqlexec(path: String, statement: String, params: BabelData) -> BabelData {
    babel_sql_run(path, statement, params, false)
}
