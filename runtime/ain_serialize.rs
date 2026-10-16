// RFC-4180-compatible CSV, UTF-8 byte sequences, and strict base64.
// Loaded only in Ain programs that reference serialization operations.
fn ain_csv_parse(input: &str) -> Result<AinData, String> {
    if input.len() > 8 * 1024 * 1024 { return Err("CSV input exceeds 8 MiB".into()); }
    if input.is_empty() { return Ok(AinData::List(Vec::new())); }
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut chars = input.chars().peekable();
    let mut quoted = false;
    let mut closed_quote = false;
    let mut at_field_start = true;
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') { chars.next(); field.push('"'); }
                else { quoted = false; closed_quote = true; }
            } else { field.push(c); }
            continue;
        }
        if closed_quote && !matches!(c, ',' | '\r' | '\n') {
            return Err("Unexpected character after quoted CSV field".into());
        }
        match c {
            '"' if at_field_start => { quoted = true; at_field_start = false; }
            '"' => return Err("Unescaped quotation mark in CSV field".into()),
            ',' => {
                row.push(AinData::Text(std::mem::take(&mut field)));
                at_field_start = true;
                closed_quote = false;
            }
            '\r' | '\n' => {
                if c == '\r' && chars.peek() == Some(&'\n') { chars.next(); }
                row.push(AinData::Text(std::mem::take(&mut field)));
                rows.push(AinData::List(std::mem::take(&mut row)));
                if rows.len() > 100_000 { return Err("Too many CSV rows".into()); }
                at_field_start = true;
                closed_quote = false;
            }
            _ => {
                if closed_quote { return Err("Trailing text after quoted field".into()); }
                field.push(c);
                at_field_start = false;
            }
        }
    }
    if quoted { return Err("Unterminated quoted CSV field".into()); }
    // A newline terminates a record; don't append a phantom empty record.
    if !(row.is_empty() && at_field_start && field.is_empty()) {
        row.push(AinData::Text(field));
        rows.push(AinData::List(row));
    } else if rows.is_empty() {
        rows.push(AinData::List(vec![AinData::Text(String::new())]));
    }
    Ok(AinData::List(rows))
}
fn ain_csvparse(input: String) -> AinData {
    ain_csv_parse(&input).unwrap_or_else(|e| panic!("Invalid CSV: {}", e))
}
fn ain_trycsv(input: String) -> AinData {
    match ain_csv_parse(&input) {
        Ok(value) => ain_result(true, value, ""),
        Err(e) => ain_result(false, AinData::Null, &e)
    }
}
fn ain_csvwrite(data: AinData) -> String {
    let AinData::List(rows) = data else { panic!("csvwrite requires a list of rows"); };
    let mut output = String::new();
    for (index, row) in rows.into_iter().enumerate() {
        let AinData::List(cols) = row else { panic!("CSV row must be a list"); };
        if index != 0 { output.push_str("\r\n"); }
        for (index, col) in cols.into_iter().enumerate() {
            if index != 0 { output.push(','); }
            let value = match col {
                AinData::Text(s) | AinData::Number(s) => s,
                AinData::Bool(b) => b.to_string(),
                AinData::Null => String::new(),
                _ => panic!("CSV cells must be scalar Data"),
            };
            if value.chars().any(|ch| matches!(ch, ',' | '"' | '\r' | '\n')) {
                output.push('"');
                for c in value.chars() { if c == '"' { output.push('"'); } output.push(c); }
                output.push('"');
            } else { output.push_str(&value); }
        }
    }
    output
}
fn ain_bytes_data(input: Vec<u8>) -> AinData {
    AinData::List(input.into_iter().map(|b| AinData::Number(b.to_string())).collect())
}
fn ain_bytes_checked(input: AinData) -> Result<Vec<u8>, String> {
    let AinData::List(values) = input else { return Err("Expected byte list".into()); };
    if values.len() > 8 * 1024 * 1024 { return Err("Byte list exceeds 8 MiB".into()); }
    values.into_iter().map(|v| match v {
        AinData::Number(n) => n.parse::<u8>().map_err(|_| "Byte must be 0..255".to_string()),
        _ => Err("Byte elements must be numeric".into())
    }).collect()
}
fn ain_utf8bytes(input: String) -> AinData { ain_bytes_data(input.into_bytes()) }
fn ain_frombytes(input: AinData) -> AinData {
    let result = ain_bytes_checked(input).and_then(|b| String::from_utf8(b).map_err(|e| e.to_string()));
    match result {
        Ok(s) => ain_result(true, AinData::Text(s), ""),
        Err(e) => ain_result(false, AinData::Null, &e),
    }
}
const AIN_B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
fn ain_b64encode(input: AinData) -> String {
    let bytes = ain_bytes_checked(input).unwrap_or_else(|e| panic!("base64 encoding: {}", e));
    let mut out = String::with_capacity(((bytes.len() + 2) / 3) * 4);
    for chunk in bytes.chunks(3) {
        let n = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | (chunk.get(2).copied().unwrap_or(0) as u32);
        out.push(AIN_B64[((n >> 18) & 63) as usize] as char);
        out.push(AIN_B64[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { AIN_B64[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { AIN_B64[(n & 63) as usize] as char } else { '=' });
    }
    out
}
fn ain_b64decode(text: String) -> AinData {
    let result: Result<Vec<u8>, String> = (|| {
        let src = text.as_bytes();
        if src.len() > 12 * 1024 * 1024 { return Err("Base64 input exceeds 12 MiB".into()); }
        if src.len() % 4 != 0 { return Err("Base64 length must be divisible by 4".into()); }
        let mut out = Vec::with_capacity(src.len() / 4 * 3);
        for (i, chunk) in src.chunks(4).enumerate() {
            let mut v = [0u32; 4];
            for j in 0..4 {
                v[j] = match chunk[j] {
                    b'A'..=b'Z' => (chunk[j] - b'A') as u32,
                    b'a'..=b'z' => (chunk[j] - b'a' + 26) as u32,
                    b'0'..=b'9' => (chunk[j] - b'0' + 52) as u32,
                    b'+' => 62, b'/' => 63,
                    b'=' if j >= 2 => 0,
                    _ => return Err("Invalid Base64 character".into()),
                };
            }
            let padding = (chunk[2] == b'=') as usize + (chunk[3] == b'=') as usize;
            if chunk[2] == b'=' && chunk[3] != b'=' { return Err("Invalid Base64 padding".into()); }
            if padding > 0 && i != src.len() / 4 - 1 { return Err("Base64 padding before end".into()); }
            if padding == 2 && v[1] & 15 != 0 { return Err("Noncanonical Base64 padding".into()); }
            if padding == 1 && v[2] & 3 != 0 { return Err("Noncanonical Base64 padding".into()); }
            let n = v[0] << 18 | v[1] << 12 | v[2] << 6 | v[3];
            out.push((n >> 16) as u8);
            if padding < 2 { out.push((n >> 8) as u8); }
            if padding == 0 { out.push(n as u8); }
        }
        Ok(out)
    })();
    match result {
        Ok(bytes) => ain_result(true, ain_bytes_data(bytes), ""),
        Err(error) => ain_result(false, AinData::Null, &error),
    }
}
