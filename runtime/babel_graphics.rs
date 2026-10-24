// Small, dependency-free 2D SVG and glTF 2.0 3D mesh exporters.
// They deliberately export graphics data; they are not interactive renderers.
fn babel_graph_number(fields: &std::collections::BTreeMap<String,BabelData>,
                    key: &str) -> Result<i64, String> {
    let Some(BabelData::Number(s)) = fields.get(key) else {
        return Err(format!("Missing numeric geometry field {key}"));
    };
    s.parse::<i64>().map_err(|_| format!("Geometry field {key} must be an integer"))
}
fn babel_svg_escape(input: &str) -> String {
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
fn babel_svg_color(fields: &std::collections::BTreeMap<String,BabelData>,
                 key: &str, fallback: &str) -> Result<String, String> {
    let color = match fields.get(key) {
        None => return Ok(fallback.into()),
        Some(BabelData::Text(s)) => s,
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
fn babel_svgwrite(path: String, scene: BabelData) -> BabelData {
    let run = (|| -> Result<BabelData, String> {
        if !babel_permission_allowed("write") { return Err("Babel capability denied: write".into()); }
        let BabelData::Object(spec) = scene else { return Err("SVG scene must be a record".into()); };
        let width = babel_graph_number(&spec, "width")?;
        let height = babel_graph_number(&spec, "height")?;
        if !(1..=8192).contains(&width) || !(1..=8192).contains(&height) {
            return Err("SVG dimensions must be 1..8192".into());
        }
        let Some(BabelData::List(shapes)) = spec.get("shapes") else {
            return Err("SVG requires a shapes list".into());
        };
        if shapes.len() > 1000 { return Err("Too many SVG shapes".into()); }
        let mut output = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"#);
        output.push('\n');
        for shape in shapes {
            let BabelData::Object(values) = shape else {
                return Err("Each SVG shape must be a record".into());
            };
            let Some(BabelData::Text(kind)) = values.get("kind") else {
                return Err("SVG shape needs a kind".into());
            };
            match kind.as_str() {
                "rect" => {
                    let x = babel_graph_number(values, "x")?;
                    let y = babel_graph_number(values, "y")?;
                    let w = babel_graph_number(values, "width")?;
                    let h = babel_graph_number(values, "height")?;
                    if w < 0 || h < 0 { return Err("Negative SVG shape size".into()); }
                    let fill = babel_svg_color(values, "fill", "black")?;
                    output.push_str(&format!(
                        r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}"/>"#));
                }
                "circle" => {
                    let x = babel_graph_number(values, "cx")?;
                    let y = babel_graph_number(values, "cy")?;
                    let radius = babel_graph_number(values, "r")?;
                    if radius < 0 { return Err("Negative SVG circle radius".into()); }
                    let fill = babel_svg_color(values, "fill", "black")?;
                    output.push_str(&format!(
                        r#"<circle cx="{x}" cy="{y}" r="{radius}" fill="{fill}"/>"#));
                }
                "text" => {
                    let x = babel_graph_number(values, "x")?;
                    let y = babel_graph_number(values, "y")?;
                    let Some(BabelData::Text(text)) = values.get("value") else {
                        return Err("SVG text requires a string value".into());
                    };
                    if text.len() > 10_000 { return Err("SVG text too long".into()); }
                    let fill = babel_svg_color(values, "fill", "black")?;
                    output.push_str(&format!(
                        r#"<text x="{x}" y="{y}" fill="{fill}">{}</text>"#,
                        babel_svg_escape(text)));
                }
                _ => return Err("Unsupported SVG shape kind".into()),
            }
            output.push('\n');
        }
        output.push_str("</svg>\n");
        std::fs::write(path, &output).map_err(|e| e.to_string())?;
        Ok(BabelData::Number(shapes.len().to_string()))
    })();
    match run { Ok(v) => babel_result(true, v, ""), Err(e) => babel_result(false, BabelData::Null, &e) }
}
fn babel_gltfwrite(path: String, scene: BabelData) -> BabelData {
    let run = (|| -> Result<BabelData, String> {
        if !babel_permission_allowed("write") { return Err("Babel capability denied: write".into()); }
        let BabelData::Object(spec) = scene else { return Err("glTF scene must be a record".into()); };
        let Some(BabelData::List(positions)) = spec.get("positions") else {
            return Err("glTF requires a positions list".into());
        };
        let Some(BabelData::List(indices)) = spec.get("indices") else {
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
            let BabelData::List(coords) = point else {
                return Err("glTF vertices must be three-item lists".into());
            };
            if coords.len() != 3 { return Err("glTF vertex must have three coordinates".into()); }
            for axis in 0..3 {
                let BabelData::Number(number) = &coords[axis] else {
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
            let BabelData::Number(number) = index else {
                return Err("glTF indices must be numeric".into());
            };
            let value = number.parse::<u32>().map_err(|_| "Invalid glTF index")?;
            if value as usize >= positions.len() { return Err("glTF index out of range".into()); }
            binary.extend_from_slice(&value.to_le_bytes());
        }
        let uri = babel_b64encode(babel_bytes_data(binary.clone()));
        let total = binary.len();
        let json = format!(
            r#"{{"asset":{{"version":"2.0","generator":"Babel"}},"buffers":[{{"uri":"data:application/octet-stream;base64,{uri}","byteLength":{total}}}],"bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":{position_bytes},"target":34962}},{{"buffer":0,"byteOffset":{position_bytes},"byteLength":{},"target":34963}}],"accessors":[{{"bufferView":0,"componentType":5126,"count":{},"type":"VEC3","min":[{},{},{}],"max":[{},{},{}]}},{{"bufferView":1,"componentType":5125,"count":{},"type":"SCALAR"}}],"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}},"indices":1,"mode":4}}]}}],"nodes":[{{"mesh":0}}],"scenes":[{{"nodes":[0]}}],"scene":0}}"#,
            indices.len() * 4, positions.len(),
            minimum[0],minimum[1],minimum[2],
            maximum[0],maximum[1],maximum[2], indices.len()
        );
        std::fs::write(path, json).map_err(|e| e.to_string())?;
        Ok(BabelData::Number(positions.len().to_string()))
    })();
    match run { Ok(v) => babel_result(true, v, ""), Err(e) => babel_result(false, BabelData::Null, &e) }
}
