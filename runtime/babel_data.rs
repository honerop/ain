// Babel's standard-library Data type: structured JSON values, persistent-style
// list/object operations, and deterministic canonical serialization.
// No third-party crates are required to compile generated Babel programs.
#[derive(Clone, Debug, PartialEq)]
enum BabelData {
    Null,
    Bool(bool),
    Number(String),
    Text(String),
    List(Vec<BabelData>),
    Object(std::collections::BTreeMap<String, BabelData>),
}
impl BabelText for BabelData {
    fn babel_text(&self) -> String { self.json() }
}
impl BabelData {
    fn json(&self) -> String {
        match self {
            Self::Null => "null".into(),
            Self::Bool(b) => b.to_string(),
            Self::Number(n) => n.clone(),
            Self::Text(s) => babel_json_quote(s),
            Self::List(values) => format!("[{}]", values.iter().map(Self::json).collect::<Vec<_>>().join(",")),
            Self::Object(values) => format!("{{{}}}", values.iter()
                .map(|(key, value)| format!("{}:{}", babel_json_quote(key), value.json()))
                .collect::<Vec<_>>().join(",")),
        }
    }
}
fn babel_json_quote(s: &str) -> String {
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
struct BabelJsonParser<'a> {
    iter: std::iter::Peekable<std::str::Chars<'a>>,
}
impl BabelJsonParser<'_> {
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
    fn value(&mut self, depth: usize) -> Result<BabelData, String> {
        if depth > 64 { return Err("JSON nesting is too deep".into()); }
        self.white();
        match self.iter.peek().copied().ok_or("Expected JSON value")? {
            'n' => { self.literal("null")?; Ok(BabelData::Null) }
            't' => { self.literal("true")?; Ok(BabelData::Bool(true)) }
            'f' => { self.literal("false")?; Ok(BabelData::Bool(false)) }
            '"' => Ok(BabelData::Text(self.quoted()?)),
            '-' | '0'..='9' => Ok(BabelData::Number(self.number()?)),
            '[' => {
                self.take('[')?;
                let mut items = Vec::new();
                self.white();
                if self.iter.peek() == Some(&']') { self.iter.next(); return Ok(BabelData::List(items)); }
                loop {
                    items.push(self.value(depth + 1)?);
                    self.white();
                    match self.iter.next() {
                        Some(']') => break,
                        Some(',') => {},
                        _ => return Err("Expected ',' or ']'".into()),
                    }
                }
                Ok(BabelData::List(items))
            }
            '{' => {
                self.take('{')?;
                let mut items = std::collections::BTreeMap::new();
                self.white();
                if self.iter.peek() == Some(&'}') { self.iter.next(); return Ok(BabelData::Object(items)); }
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
                Ok(BabelData::Object(items))
            }
            _ => Err("Unexpected JSON token".into())
        }
    }
}
fn babel_json_parse(source: String) -> Result<BabelData, String> {
    if source.len() > 8 * 1024 * 1024 { return Err("JSON input exceeds 8 MiB".into()); }
    let mut p = BabelJsonParser { iter: source.chars().peekable() };
    let parsed = p.value(0)?;
    p.white();
    if p.iter.next().is_some() { return Err("Unexpected trailing JSON data".into()); }
    Ok(parsed)
}
fn babel_json(source: String) -> BabelData {
    babel_json_parse(source).unwrap_or_else(|error| panic!("Invalid JSON: {}", error))
}
fn babel_tryjson(source: String) -> BabelData {
    match babel_json_parse(source) {
        Ok(value) => babel_result(true, value, ""),
        Err(error) => babel_result(false, BabelData::Null, &error),
    }
}
fn babel_result(ok: bool, value: BabelData, error: &str) -> BabelData {
    let mut fields = std::collections::BTreeMap::new();
    fields.insert("ok".into(), BabelData::Bool(ok));
    fields.insert("value".into(), value);
    fields.insert("error".into(), BabelData::Text(error.into()));
    BabelData::Object(fields)
}
fn babel_data_array(_: ()) -> BabelData { BabelData::List(Vec::new()) }
fn babel_data_record(_: ()) -> BabelData { BabelData::Object(std::collections::BTreeMap::new()) }
fn babel_data_null(_: ()) -> BabelData { BabelData::Null }
fn babel_data_int(v: i64) -> BabelData { BabelData::Number(v.to_string()) }
fn babel_data_text(v: String) -> BabelData { BabelData::Text(v) }
fn babel_data_bool(v: bool) -> BabelData { BabelData::Bool(v) }
fn babel_data_pair(a: BabelData, b: BabelData) -> BabelData { BabelData::List(vec![a, b]) }
fn babel_data_push(value: BabelData, item: BabelData) -> BabelData {
    match value { BabelData::List(mut items) => { items.push(item); BabelData::List(items) },
        _ => panic!("push requires list") }
}
fn babel_data_get(value: BabelData, index: i64) -> BabelData {
    match value { BabelData::List(items) => usize::try_from(index).ok()
        .and_then(|i| items.get(i).cloned()).unwrap_or(BabelData::Null),
        _ => panic!("get requires list") }
}
fn babel_data_put(value: BabelData, key: String, item: BabelData) -> BabelData {
    match value { BabelData::Object(mut fields) => {
        fields.insert(key, item); BabelData::Object(fields)
    }, _ => panic!("put requires record") }
}
fn babel_data_field(value: BabelData, key: String) -> BabelData {
    match value { BabelData::Object(fields) => fields.get(&key).cloned().unwrap_or(BabelData::Null),
        _ => panic!("field requires record") }
}
fn babel_data_size(value: BabelData) -> i64 {
    match value { BabelData::List(items) => items.len() as i64,
        BabelData::Object(fields) => fields.len() as i64, _ => panic!("size requires list or record") }
}
fn babel_data_keys(value: BabelData) -> BabelData {
    match value { BabelData::Object(fields) => BabelData::List(fields.keys()
        .map(|key| BabelData::Text(key.clone())).collect()), _ => panic!("keys requires record") }
}
fn babel_data_has(value: BabelData, key: String) -> bool {
    match value { BabelData::Object(fields) => fields.contains_key(&key),
        _ => panic!("has requires record") }
}
fn babel_data_asint(value: BabelData) -> i64 {
    match value { BabelData::Number(v) => v.parse::<i64>().expect("integer out of range"),
        _ => panic!("Expected numeric value") }
}
fn babel_data_astext(value: BabelData) -> String {
    match value { BabelData::Text(v) => v, _ => panic!("Expected text value") }
}
fn babel_data_asbool(value: BabelData) -> bool {
    match value { BabelData::Bool(v) => v, _ => panic!("Expected boolean value") }
}
fn babel_data_map<F: Fn(BabelData) -> BabelData>(list: BabelData, f: F) -> BabelData {
    match list { BabelData::List(items) => BabelData::List(items.into_iter().map(f).collect()),
        _ => panic!("map requires list") }
}
fn babel_data_filter<F: Fn(BabelData) -> bool>(list: BabelData, f: F) -> BabelData {
    match list { BabelData::List(items) => BabelData::List(items.into_iter()
        .filter(|item| f(item.clone())).collect()), _ => panic!("filter requires list") }
}
fn babel_data_fold<F: Fn(BabelData, BabelData) -> BabelData>(
        list: BabelData, init: BabelData, f: F) -> BabelData {
    match list { BabelData::List(items) => items.into_iter().fold(init, f),
        _ => panic!("fold requires list") }
}

type BabelCell = std::rc::Rc<std::cell::RefCell<BabelData>>;
impl BabelText for BabelCell {
    fn babel_text(&self) -> String { self.borrow().json() }
}
fn babel_data_state(value: BabelData) -> BabelCell {
    std::rc::Rc::new(std::cell::RefCell::new(value))
}
fn babel_data_readstate(cell: BabelCell) -> BabelData {
    cell.borrow().clone()
}
fn babel_data_setstate(cell: BabelCell, value: BabelData) {
    *cell.borrow_mut() = value;
}
fn babel_data_swapstate(cell: BabelCell, value: BabelData) -> BabelData {
    std::mem::replace(&mut *cell.borrow_mut(), value)
}
fn babel_data_iterate<F: Fn(BabelData) -> BabelData>(
        n: i64, mut value: BabelData, f: F) -> BabelData {
    assert!((0..=1_000_000).contains(&n), "iterate must be between 0 and 1,000,000");
    for _ in 0..n { value = f(value); }
    value
}
fn babel_data_each<F: Fn(BabelData)>(list: BabelData, f: F) {
    match list {
        BabelData::List(items) => for item in items { f(item); },
        _ => panic!("each requires list")
    }
}
