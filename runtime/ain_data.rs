// Ain's standard-library Data type: structured JSON values, persistent-style
// list/object operations, and deterministic canonical serialization.
// No third-party crates are required to compile generated Ain programs.
#[derive(Clone, Debug, PartialEq)]
enum AinData {
    Null,
    Bool(bool),
    Number(String),
    Text(String),
    List(Vec<AinData>),
    Object(std::collections::BTreeMap<String, AinData>),
}
impl AinText for AinData {
    fn ain_text(&self) -> String { self.json() }
}
impl AinData {
    fn json(&self) -> String {
        match self {
            Self::Null => "null".into(),
            Self::Bool(b) => b.to_string(),
            Self::Number(n) => n.clone(),
            Self::Text(s) => ain_json_quote(s),
            Self::List(values) => format!("[{}]", values.iter().map(Self::json).collect::<Vec<_>>().join(",")),
            Self::Object(values) => format!("{{{}}}", values.iter()
                .map(|(key, value)| format!("{}:{}", ain_json_quote(key), value.json()))
                .collect::<Vec<_>>().join(",")),
        }
    }
}
fn ain_json_quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            c if c <= '\u{001F}' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
struct AinJsonParser<'a> {
    iter: std::iter::Peekable<std::str::Chars<'a>>,
}
impl AinJsonParser<'_> {
    fn white(&mut self) {
        while matches!(self.iter.peek(), Some(' ' | '\t' | '\r' | '\n')) { self.iter.next(); }
    }
    fn take(&mut self, ch: char) -> Result<(), String> {
        self.white();
        if self.iter.next() == Some(ch) { Ok(()) } else { Err(format!("Expected '{}'", ch)) }
    }
    fn literal(&mut self, word: &str) -> Result<(), String> {
        for c in word.chars() {
            if self.iter.next() != Some(c) { return Err(format!("Expected {word}")); }
        }
        Ok(())
    }
    fn quoted(&mut self) -> Result<String, String> {
        self.take('"')?;
        let mut value = String::new();
        loop {
            let c = self.iter.next().ok_or("Unterminated string")?;
            match c {
                '"' => return Ok(value),
                '\\' => match self.iter.next().ok_or("Unterminated escape")? {
                    '"' => value.push('"'),
                    '\\' => value.push('\\'),
                    '/' => value.push('/'),
                    'b' => value.push('\u{0008}'),
                    'f' => value.push('\u{000c}'),
                    'n' => value.push('\n'),
                    'r' => value.push('\r'),
                    't' => value.push('\t'),
                    'u' => {
                        let first = self.hex4()?;
                        let code = if (0xD800..=0xDBFF).contains(&first) {
                            if self.iter.next() != Some('\\') || self.iter.next() != Some('u') {
                                return Err("Missing UTF-16 low surrogate".into());
                            }
                            let second = self.hex4()?;
                            if !(0xDC00..=0xDFFF).contains(&second) {
                                return Err("Invalid UTF-16 low surrogate".into());
                            }
                            0x10000 + (first - 0xD800) * 1024 + second - 0xDC00
                        } else {
                            if (0xDC00..=0xDFFF).contains(&first) {
                                return Err("Unexpected UTF-16 low surrogate".into());
                            }
                            first
                        };
                        value.push(char::from_u32(code).ok_or("Invalid Unicode scalar")?);
                    }
                    _ => return Err("Invalid JSON escape".into()),
                },
                c if c < '\u{0020}' => return Err("Control character in JSON string".into()),
                c => value.push(c),
            }
        }
    }
    fn hex4(&mut self) -> Result<u32, String> {
        let mut n = 0;
        for _ in 0..4 {
            n = n * 16 + self.iter.next().and_then(|c| c.to_digit(16))
                .ok_or("Invalid four-digit Unicode escape")?;
        }
        Ok(n)
    }
    fn number(&mut self) -> Result<String, String> {
        let mut value = String::new();
        if self.iter.peek() == Some(&'-') { value.push(self.iter.next().unwrap()); }
        match self.iter.next() {
            Some('0') => value.push('0'),
            Some(c @ '1'..='9') => {
                value.push(c);
                while matches!(self.iter.peek(), Some('0'..='9')) {
                    value.push(self.iter.next().unwrap());
                }
            }
            _ => return Err("Invalid JSON number".into()),
        }
        if self.iter.peek() == Some(&'.') {
            value.push(self.iter.next().unwrap());
            let mut digits = 0;
            while matches!(self.iter.peek(), Some('0'..='9')) {
                digits += 1;
                value.push(self.iter.next().unwrap());
            }
            if digits == 0 { return Err("Decimal point without digits".into()); }
        }
        if matches!(self.iter.peek(), Some('e' | 'E')) {
            value.push(self.iter.next().unwrap());
            if matches!(self.iter.peek(), Some('+' | '-')) {
                value.push(self.iter.next().unwrap());
            }
            let mut digits = 0;
            while matches!(self.iter.peek(), Some('0'..='9')) {
                digits += 1;
                value.push(self.iter.next().unwrap());
            }
            if digits == 0 { return Err("Exponent without digits".into()); }
        }
        Ok(value)
    }
    fn value(&mut self, depth: usize) -> Result<AinData, String> {
        if depth > 64 { return Err("JSON nesting is too deep".into()); }
        self.white();
        match self.iter.peek().copied().ok_or("Expected JSON value")? {
            'n' => { self.literal("null")?; Ok(AinData::Null) }
            't' => { self.literal("true")?; Ok(AinData::Bool(true)) }
            'f' => { self.literal("false")?; Ok(AinData::Bool(false)) }
            '"' => Ok(AinData::Text(self.quoted()?)),
            '-' | '0'..='9' => Ok(AinData::Number(self.number()?)),
            '[' => {
                self.take('[')?;
                let mut items = Vec::new();
                self.white();
                if self.iter.peek() == Some(&']') { self.iter.next(); return Ok(AinData::List(items)); }
                loop {
                    items.push(self.value(depth + 1)?);
                    self.white();
                    match self.iter.next() {
                        Some(']') => break,
                        Some(',') => {},
                        _ => return Err("Expected ',' or ']'".into()),
                    }
                }
                Ok(AinData::List(items))
            }
            '{' => {
                self.take('{')?;
                let mut items = std::collections::BTreeMap::new();
                self.white();
                if self.iter.peek() == Some(&'}') { self.iter.next(); return Ok(AinData::Object(items)); }
                loop {
                    let key = self.quoted()?;
                    self.take(':')?;
                    let value = self.value(depth + 1)?;
                    items.insert(key, value);
                    self.white();
                    match self.iter.next() {
                        Some('}') => break,
                        Some(',') => {},
                        _ => return Err("Expected ',' or '}'".into()),
                    }
                }
                Ok(AinData::Object(items))
            }
            _ => Err("Unexpected JSON token".into())
        }
    }
}
fn ain_json_parse(source: String) -> Result<AinData, String> {
    if source.len() > 8 * 1024 * 1024 { return Err("JSON input exceeds 8 MiB".into()); }
    let mut p = AinJsonParser { iter: source.chars().peekable() };
    let parsed = p.value(0)?;
    p.white();
    if p.iter.next().is_some() { return Err("Unexpected trailing JSON data".into()); }
    Ok(parsed)
}
fn ain_json(source: String) -> AinData {
    ain_json_parse(source).unwrap_or_else(|error| panic!("Invalid JSON: {}", error))
}
fn ain_tryjson(source: String) -> AinData {
    match ain_json_parse(source) {
        Ok(value) => ain_result(true, value, ""),
        Err(error) => ain_result(false, AinData::Null, &error),
    }
}
fn ain_result(ok: bool, value: AinData, error: &str) -> AinData {
    let mut fields = std::collections::BTreeMap::new();
    fields.insert("ok".into(), AinData::Bool(ok));
    fields.insert("value".into(), value);
    fields.insert("error".into(), AinData::Text(error.into()));
    AinData::Object(fields)
}
fn ain_data_array(_: ()) -> AinData { AinData::List(Vec::new()) }
fn ain_data_record(_: ()) -> AinData { AinData::Object(std::collections::BTreeMap::new()) }
fn ain_data_null(_: ()) -> AinData { AinData::Null }
fn ain_data_int(v: i64) -> AinData { AinData::Number(v.to_string()) }
fn ain_data_text(v: String) -> AinData { AinData::Text(v) }
fn ain_data_bool(v: bool) -> AinData { AinData::Bool(v) }
fn ain_data_pair(a: AinData, b: AinData) -> AinData { AinData::List(vec![a, b]) }
fn ain_data_push(value: AinData, item: AinData) -> AinData {
    match value { AinData::List(mut items) => { items.push(item); AinData::List(items) },
        _ => panic!("push requires list") }
}
fn ain_data_get(value: AinData, index: i64) -> AinData {
    match value { AinData::List(items) => usize::try_from(index).ok()
        .and_then(|i| items.get(i).cloned()).unwrap_or(AinData::Null),
        _ => panic!("get requires list") }
}
fn ain_data_put(value: AinData, key: String, item: AinData) -> AinData {
    match value { AinData::Object(mut fields) => {
        fields.insert(key, item); AinData::Object(fields)
    }, _ => panic!("put requires record") }
}
fn ain_data_field(value: AinData, key: String) -> AinData {
    match value { AinData::Object(fields) => fields.get(&key).cloned().unwrap_or(AinData::Null),
        _ => panic!("field requires record") }
}
fn ain_data_size(value: AinData) -> i64 {
    match value { AinData::List(items) => items.len() as i64,
        AinData::Object(fields) => fields.len() as i64, _ => panic!("size requires list or record") }
}
fn ain_data_keys(value: AinData) -> AinData {
    match value { AinData::Object(fields) => AinData::List(fields.keys()
        .map(|key| AinData::Text(key.clone())).collect()), _ => panic!("keys requires record") }
}
fn ain_data_has(value: AinData, key: String) -> bool {
    match value { AinData::Object(fields) => fields.contains_key(&key),
        _ => panic!("has requires record") }
}
fn ain_data_asint(value: AinData) -> i64 {
    match value { AinData::Number(v) => v.parse::<i64>().expect("integer out of range"),
        _ => panic!("Expected numeric value") }
}
fn ain_data_astext(value: AinData) -> String {
    match value { AinData::Text(v) => v, _ => panic!("Expected text value") }
}
fn ain_data_asbool(value: AinData) -> bool {
    match value { AinData::Bool(v) => v, _ => panic!("Expected boolean value") }
}
fn ain_data_map<F: Fn(AinData) -> AinData>(list: AinData, f: F) -> AinData {
    match list { AinData::List(items) => AinData::List(items.into_iter().map(f).collect()),
        _ => panic!("map requires list") }
}
fn ain_data_filter<F: Fn(AinData) -> bool>(list: AinData, f: F) -> AinData {
    match list { AinData::List(items) => AinData::List(items.into_iter()
        .filter(|item| f(item.clone())).collect()), _ => panic!("filter requires list") }
}
fn ain_data_fold<F: Fn(AinData, AinData) -> AinData>(
        list: AinData, init: AinData, f: F) -> AinData {
    match list { AinData::List(items) => items.into_iter().fold(init, f),
        _ => panic!("fold requires list") }
}

type AinCell = std::rc::Rc<std::cell::RefCell<AinData>>;
impl AinText for AinCell {
    fn ain_text(&self) -> String { self.borrow().json() }
}
fn ain_data_state(value: AinData) -> AinCell {
    std::rc::Rc::new(std::cell::RefCell::new(value))
}
fn ain_data_readstate(cell: AinCell) -> AinData {
    cell.borrow().clone()
}
fn ain_data_setstate(cell: AinCell, value: AinData) {
    *cell.borrow_mut() = value;
}
fn ain_data_swapstate(cell: AinCell, value: AinData) -> AinData {
    std::mem::replace(&mut *cell.borrow_mut(), value)
}
fn ain_data_iterate<F: Fn(AinData) -> AinData>(
        n: i64, mut value: AinData, f: F) -> AinData {
    assert!((0..=1_000_000).contains(&n), "iterate must be between 0 and 1,000,000");
    for _ in 0..n { value = f(value); }
    value
}
fn ain_data_each<F: Fn(AinData)>(list: AinData, f: F) {
    match list {
        AinData::List(items) => for item in items { f(item); },
        _ => panic!("each requires list")
    }
}
