// Small, dependency-free 2D SVG and glTF 2.0 3D mesh exporters.
// They deliberately export graphics data; they are not interactive renderers.
fn ain_graph_number(fields: &std::collections::BTreeMap<String,AinData>,
                    key: &str) -> Result<i64, String> {
    let Some(AinData::Number(s)) = fields.get(key) else {
        return Err(format!("Missing numeric geometry field {key}"));
    };
    s.parse::<i64>().map_err(|_| format!("Geometry field {key} must be an integer"))
}
fn ain_svg_escape(input: &str) -> String {
    let mut result = String::new();
    for c in input.chars() {
        match c {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&apos;"),
            c if c >= ' ' || c == '\n' || c == '\t' => result.push(c),
            _ => {}
        }
    }
    result
}
fn ain_svg_color(fields: &std::collections::BTreeMap<String,AinData>,
                 key: &str, fallback: &str) -> Result<String, String> {
    let color = match fields.get(key) {
        None => return Ok(fallback.into()),
        Some(AinData::Text(s)) => s,
        _ => return Err("SVG color must be text".into())
    };
    let ok = if let Some(hex) = color.strip_prefix('#') {
        [3,6].contains(&hex.len()) && hex.bytes().all(|b| b.is_ascii_hexdigit())
    } else {
        color.len() <= 32 && !color.is_empty()
            && color.bytes().all(|b| b.is_ascii_alphabetic())
    };
    if !ok { return Err("Invalid SVG color".into()); }
    Ok(color.clone())
}
fn ain_svgwrite(path: String, scene: AinData) -> AinData {
    let run = (|| -> Result<AinData, String> {
        if !ain_permission_allowed("write") { return Err("Ain capability denied: write".into()); }
        let AinData::Object(spec) = scene else { return Err("SVG scene must be a record".into()); };
        let width = ain_graph_number(&spec, "width")?;
        let height = ain_graph_number(&spec, "height")?;
        if !(1..=8192).contains(&width) || !(1..=8192).contains(&height) {
            return Err("SVG dimensions must be 1..8192".into());
        }
        let Some(AinData::List(shapes)) = spec.get("shapes") else {
            return Err("SVG requires a shapes list".into());
        };
        if shapes.len() > 1000 { return Err("Too many SVG shapes".into()); }
        let mut output = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"#);
        output.push('\n');
        for shape in shapes {
            let AinData::Object(values) = shape else {
                return Err("Each SVG shape must be a record".into());
            };
            let Some(AinData::Text(kind)) = values.get("kind") else {
                return Err("SVG shape needs a kind".into());
            };
            match kind.as_str() {
                "rect" => {
                    let x = ain_graph_number(values, "x")?;
                    let y = ain_graph_number(values, "y")?;
                    let w = ain_graph_number(values, "width")?;
                    let h = ain_graph_number(values, "height")?;
                    if w < 0 || h < 0 { return Err("Negative SVG shape size".into()); }
                    let fill = ain_svg_color(values, "fill", "black")?;
                    output.push_str(&format!(
                        r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}"/>"#));
                }
                "circle" => {
                    let x = ain_graph_number(values, "cx")?;
                    let y = ain_graph_number(values, "cy")?;
                    let radius = ain_graph_number(values, "r")?;
                    if radius < 0 { return Err("Negative SVG circle radius".into()); }
                    let fill = ain_svg_color(values, "fill", "black")?;
                    output.push_str(&format!(
                        r#"<circle cx="{x}" cy="{y}" r="{radius}" fill="{fill}"/>"#));
                }
                "line" => {
                    let x1 = ain_graph_number(values, "x1")?;
                    let y1 = ain_graph_number(values, "y1")?;
                    let x2 = ain_graph_number(values, "x2")?;
                    let y2 = ain_graph_number(values, "y2")?;
                    let stroke = ain_svg_color(values, "stroke", "white")?;
                    let stroke_width = match values.get("stroke_width") {
                        None => 3,
                        Some(_) => ain_graph_number(values, "stroke_width")?,
                    };
                    if !(1..=128).contains(&stroke_width) {
                        return Err("SVG line stroke_width must be 1..128".into());
                    }
                    output.push_str(&format!(
                        r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{stroke}" stroke-width="{stroke_width}" stroke-linecap="round"/>"#
                    ));
                }
                "image" => {
                    let x = ain_graph_number(values, "x")?;
                    let y = ain_graph_number(values, "y")?;
                    let width = ain_graph_number(values, "width")?;
                    let height = ain_graph_number(values, "height")?;
                    if width <= 0 || height <= 0 {
                        return Err("SVG image dimensions must be positive".into());
                    }
                    let Some(AinData::Text(uri)) = values.get("data") else {
                        return Err("SVG image requires an embedded data URI".into());
                    };
                    let encoded = uri.strip_prefix("data:image/png;base64,")
                        .or_else(|| uri.strip_prefix("data:image/jpeg;base64,"))
                        .ok_or_else(|| "SVG images must embed PNG or JPEG".to_string())?;
                    if encoded.is_empty() || encoded.len() > 1_400_000
                        || !encoded.bytes().all(|c| c.is_ascii_alphanumeric()
                            || c == b'+' || c == b'/' || c == b'=') {
                        return Err("Invalid or oversized SVG image data".into());
                    }
                    output.push_str(&format!(
                        r#"<image x="{x}" y="{y}" width="{width}" height="{height}" preserveAspectRatio="xMidYMid meet" href="{uri}"/>"#
                    ));
                }
                "text" => {
                    let x = ain_graph_number(values, "x")?;
                    let y = ain_graph_number(values, "y")?;
                    let Some(AinData::Text(text)) = values.get("value") else {
                        return Err("SVG text requires a string value".into());
                    };
                    if text.len() > 10_000 { return Err("SVG text too long".into()); }
                    let fill = ain_svg_color(values, "fill", "black")?;
                    let font_size = match values.get("font_size") {
                        None => 24,
                        Some(_) => ain_graph_number(values, "font_size")?,
                    };
                    if !(1..=512).contains(&font_size) {
                        return Err("SVG font_size must be 1..512".into());
                    }
                    let anchor = match values.get("anchor") {
                        None => "start",
                        Some(AinData::Text(s)) if s == "start" => "start",
                        Some(AinData::Text(s)) if s == "middle" => "middle",
                        Some(AinData::Text(s)) if s == "end" => "end",
                        _ => return Err("SVG anchor must be start, middle or end".into()),
                    };
                    output.push_str(&format!(
                        r#"<text x="{x}" y="{y}" fill="{fill}" font-size="{font_size}" font-family="sans-serif" text-anchor="{anchor}">{}</text>"#,
                        ain_svg_escape(text)));
                }
                _ => return Err("Unsupported SVG shape kind".into()),
            }
            output.push('\n');
        }
        output.push_str("</svg>\n");
        std::fs::write(path, &output).map_err(|e| e.to_string())?;
        Ok(AinData::Number(shapes.len().to_string()))
    })();
    match run { Ok(v) => ain_result(true, v, ""), Err(e) => ain_result(false, AinData::Null, &e) }
}
fn ain_gltfwrite(path: String, scene: AinData) -> AinData {
    let run = (|| -> Result<AinData, String> {
        if !ain_permission_allowed("write") { return Err("Ain capability denied: write".into()); }
        let AinData::Object(spec) = scene else { return Err("glTF scene must be a record".into()); };
        let Some(AinData::List(positions)) = spec.get("positions") else {
            return Err("glTF requires a positions list".into());
        };
        let Some(AinData::List(indices)) = spec.get("indices") else {
            return Err("glTF requires an indices list".into());
        };
        if positions.is_empty() || positions.len() > 20_000 ||
            indices.len() < 3 || indices.len() > 100_000 || indices.len() % 3 != 0 {
            return Err("glTF geometry count invalid".into());
        }
        let mut binary = Vec::<u8>::with_capacity(positions.len() * 12 + indices.len() * 4);
        let mut minimum = [f32::INFINITY; 3];
        let mut maximum = [f32::NEG_INFINITY; 3];
        for point in positions {
            let AinData::List(coords) = point else {
                return Err("glTF vertices must be three-item lists".into());
            };
            if coords.len() != 3 { return Err("glTF vertex must have three coordinates".into()); }
            for axis in 0..3 {
                let AinData::Number(number) = &coords[axis] else {
                    return Err("glTF coordinates must be numeric".into());
                };
                let value = number.parse::<f32>().map_err(|_| "Invalid glTF coordinate")?;
                if !value.is_finite() || value.abs() > 1_000_000.0 {
                    return Err("glTF coordinate exceeds supported range".into());
                }
                minimum[axis] = minimum[axis].min(value);
                maximum[axis] = maximum[axis].max(value);
                binary.extend_from_slice(&value.to_le_bytes());
            }
        }
        let position_bytes = binary.len();
        for index in indices {
            let AinData::Number(number) = index else {
                return Err("glTF indices must be numeric".into());
            };
            let value = number.parse::<u32>().map_err(|_| "Invalid glTF index")?;
            if value as usize >= positions.len() { return Err("glTF index out of range".into()); }
            binary.extend_from_slice(&value.to_le_bytes());
        }
        let uri = ain_b64encode(ain_bytes_data(binary.clone()));
        let total = binary.len();
        let json = format!(
            r#"{{"asset":{{"version":"2.0","generator":"Ain"}},"buffers":[{{"uri":"data:application/octet-stream;base64,{uri}","byteLength":{total}}}],"bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":{position_bytes},"target":34962}},{{"buffer":0,"byteOffset":{position_bytes},"byteLength":{},"target":34963}}],"accessors":[{{"bufferView":0,"componentType":5126,"count":{},"type":"VEC3","min":[{},{},{}],"max":[{},{},{}]}},{{"bufferView":1,"componentType":5125,"count":{},"type":"SCALAR"}}],"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}},"indices":1,"mode":4}}]}}],"nodes":[{{"mesh":0}}],"scenes":[{{"nodes":[0]}}],"scene":0}}"#,
            indices.len() * 4, positions.len(),
            minimum[0],minimum[1],minimum[2],
            maximum[0],maximum[1],maximum[2], indices.len()
        );
        std::fs::write(path, json).map_err(|e| e.to_string())?;
        Ok(AinData::Number(positions.len().to_string()))
    })();
    match run { Ok(v) => ain_result(true, v, ""), Err(e) => ain_result(false, AinData::Null, &e) }
}
