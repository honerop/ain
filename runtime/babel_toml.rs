// Strict, deterministic subset of TOML 1.0, without extra crates.
// Supports bare dotted keys, tables, strings, numbers, bools and nested arrays.
// Unsupported constructs (dates, array-of-tables, multiline strings, inline
// tables and quoted keys) return recoverable errors rather than guessed data.
fn babel_toml_key(key: &str) -> Result<Vec<String>, String> {
    let parts: Vec<_> = key.split('.').map(str::trim).collect();
    if parts.is_empty() || parts.iter().any(|p| p.is_empty() || p.len() > 128 ||
        !p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')) {
        return Err("Invalid TOML bare key".into());
    }
    Ok(parts.iter().map(|s| (*s).into()).collect())
}
fn babel_toml_uncomment(line: &str) -> String {
    let mut result = String::new();
    let mut quote = None;
    let mut escaped = false;
    for c in line.chars() {
        if let Some(q) = quote {
            result.push(c);
            if q == '"' && escaped { escaped = false; }
            else if q == '"' && c == '\\' { escaped = true; }
            else if c == q { quote = None; }
        } else if c == '"' || c == '\'' {
            quote = Some(c);
            result.push(c);
        } else if c == '#' { break; }
        else { result.push(c); }
    }
    result.trim().into()
}
fn babel_toml_value(input: &str, depth: usize) -> Result<BabelData, String> {
    if depth > 64 { return Err("TOML array nesting too deep".into()); }
    let input = input.trim();
    if input.is_empty() { return Err("Empty TOML value".into()); }
    if input.starts_with('"') {
        return match babel_json_parse(input.to_string())? {
            BabelData::Text(t) => Ok(BabelData::Text(t)),
            _ => Err("TOML string expected".into())
        };
    }
    if input.starts_with('\'') {
        if input.len() < 2 || !input.ends_with('\'') ||
            input[1..input.len()-1].contains('\'') {
            return Err("Invalid literal TOML string".into());
        }
        return Ok(BabelData::Text(input[1..input.len()-1].into()));
    }
    if input.starts_with('[') {
        if !input.ends_with(']') { return Err("Unterminated TOML array".into()); }
        let inside = &input[1..input.len()-1];
        let mut values = Vec::new();
        let mut start = 0;
        let mut quote = None;
        let mut escaped = false;
        let mut nested = 0usize;
        for (i,ch) in inside.char_indices() {
            if let Some(q) = quote {
                if q == '"' && escaped { escaped = false; }
                else if q == '"' && ch == '\\' { escaped = true; }
                else if ch == q { quote = None; }
            } else if ch == '"' || ch == '\'' { quote = Some(ch); }
            else if ch == '[' { nested += 1; }
            else if ch == ']' {
                if nested == 0 { return Err("Unbalanced TOML brackets".into()); }
                nested -= 1;
            } else if ch == ',' && nested == 0 {
                let value = inside[start..i].trim();
                if value.is_empty() { return Err("Empty TOML array element".into()); }
                values.push(babel_toml_value(value, depth+1)?);
                if values.len() > 100_000 { return Err("Too many TOML array values".into()); }
                start = i + 1;
            }
        }
        if quote.is_some() || nested != 0 { return Err("Unterminated TOML array element".into()); }
        let last = inside[start..].trim();
        if !last.is_empty() { values.push(babel_toml_value(last, depth+1)?); }
        return Ok(BabelData::List(values));
    }
    if input == "true" { return Ok(BabelData::Bool(true)); }
    if input == "false" { return Ok(BabelData::Bool(false)); }
    let numeric = input.strip_prefix('+').unwrap_or(input);
    if numeric.chars().all(|c| c.is_ascii_digit() || c == '.' || c == 'e' ||
        c == 'E' || c == '+' || c == '-' || c == '_') {
        for (i,b) in numeric.as_bytes().iter().enumerate() {
            if *b == b'_' && (i == 0 || i+1 == numeric.len() ||
                !numeric.as_bytes()[i-1].is_ascii_digit() ||
                !numeric.as_bytes()[i+1].is_ascii_digit()) {
                return Err("Invalid underscore in TOML number".into());
            }
        }
        let cleaned = numeric.replace('_',"");
        if let Ok(value) = babel_json_parse(cleaned.clone()) {
            if let BabelData::Number(_) = value {
                return Ok(BabelData::Number(cleaned));
            }
        }
    }
    Err("Unsupported or invalid TOML value".into())
}
fn babel_toml_insert(values: &mut std::collections::BTreeMap<String,BabelData>,
                  path: &[String], value: BabelData) -> Result<(), String> {
    let (head, tail) = path.split_first().ok_or("Empty TOML key")?;
    if tail.is_empty() {
        if values.contains_key(head) { return Err(format!("Duplicate TOML key: {head}")); }
        values.insert(head.clone(), value);
        return Ok(());
    }
    let next = values.entry(head.clone()).or_insert_with(
        || BabelData::Object(std::collections::BTreeMap::new()));
    let BabelData::Object(nested) = next else {
        return Err("TOML table conflicts with a scalar key".into());
    };
    babel_toml_insert(nested, tail, value)
}
fn babel_toml_parse(input: String) -> Result<BabelData, String> {
    if input.len() > 4 * 1024 * 1024 { return Err("TOML input exceeds 4 MiB".into()); }
    let mut root = std::collections::BTreeMap::new();
    let mut section: Vec<String> = Vec::new();
    let mut seen_tables = std::collections::BTreeSet::<String>::new();
    for (idx,line) in input.lines().enumerate() {
        let line = babel_toml_uncomment(line);
        if line.is_empty() { continue; }
        let parsed = (|| -> Result<(),String> {
            if line.starts_with('[') {
                if !line.ends_with(']') || line.starts_with("[[") {
                    return Err("Unsupported TOML table declaration".into());
                }
                section = babel_toml_key(&line[1..line.len()-1])?;
                if !seen_tables.insert(section.join(".")) {
                    return Err("Duplicate TOML table declaration".into());
                }
                let mut path = section.clone();
                path.push("__babel_internal_table_marker__".into());
                // Ensure table shape exists, but don't create an internal key.
                let mut current = &mut root;
                for key in section.iter() {
                    let next = current.entry(key.clone()).or_insert_with(
                        || BabelData::Object(std::collections::BTreeMap::new()));
                    let BabelData::Object(map) = next else {
                        return Err("TOML table conflicts with scalar key".into());
                    };
                    current = map;
                }
                return Ok(());
            }
            let Some((key,value)) = line.split_once('=') else {
                return Err("Expected TOML key = value".into());
            };
            let mut path = section.clone();
            path.extend(babel_toml_key(key.trim())?);
            babel_toml_insert(&mut root, &path, babel_toml_value(value, 0)?)
        })();
        parsed.map_err(|e| format!("TOML line {}: {e}", idx+1))?;
    }
    Ok(BabelData::Object(root))
}
fn babel_tomlparse(input: String) -> BabelData {
    match babel_toml_parse(input) {
        Ok(value) => babel_result(true,value,""),
        Err(e) => babel_result(false,BabelData::Null,&e),
    }
}
fn babel_toml_scalar(value: &BabelData) -> Result<String,String> {
    match value {
        BabelData::Text(t) => Ok(babel_json_quote(t)),
        BabelData::Bool(v) => Ok(v.to_string()),
        BabelData::Number(n) => {
            let numeric = babel_toml_value(n,0)?;
            if matches!(numeric,BabelData::Number(_)) { Ok(n.clone()) }
            else { Err("Invalid TOML numeric value".into()) }
        },
        BabelData::List(items) => {
            let mut encoded = Vec::new();
            for v in items { encoded.push(babel_toml_scalar(v)?); }
            Ok(format!("[{}]", encoded.join(", ")))
        },
        _ => Err("TOML cannot serialize null or inline records".into())
    }
}
fn babel_toml_sections(path: &[String], fields: &std::collections::BTreeMap<String,BabelData>,
                     out: &mut String) -> Result<(),String> {
    if !path.is_empty() { out.push_str(&format!("[{}]\n",path.join("."))); }
    for (key,value) in fields {
        babel_toml_key(key)?;
        if !matches!(value,BabelData::Object(_)) {
            out.push_str(&format!("{} = {}\n",key,babel_toml_scalar(value)?));
        }
    }
    if !path.is_empty() { out.push('\n'); }
    for (key,value) in fields {
        if let BabelData::Object(nested) = value {
            let mut next = path.to_vec(); next.push(key.clone());
            babel_toml_sections(&next,nested,out)?;
        }
    }
    Ok(())
}
fn babel_tomlwrite(data: BabelData) -> BabelData {
    let run = (|| -> Result<BabelData,String> {
        let BabelData::Object(root) = data else { return Err("TOML root must be a record".into()); };
        let mut text = String::new();
        babel_toml_sections(&[],&root,&mut text)?;
        if text.len() > 4 * 1024 * 1024 { return Err("TOML output exceeds 4 MiB".into()); }
        Ok(BabelData::Text(text))
    })();
    match run { Ok(v) => babel_result(true,v,""), Err(e) => babel_result(false,BabelData::Null,&e) }
}
