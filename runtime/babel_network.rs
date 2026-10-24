// TCP request/response client and loopback-only one-request HTTP server.
// Standard Rust networking; no shell process and no new dependencies.
fn babel_tcp_request(address: String, payload: String) -> BabelData {
    let run = (|| -> Result<BabelData, String> {
        if !babel_permission_allowed("network") { return Err("Babel capability denied: network".into()); }
        if payload.len() > 64 * 1024 { return Err("TCP request exceeds 64 KiB".into()); }
        if address.len() > 1024 { return Err("TCP address too long".into()); }
        use std::io::{Read, Write};
        use std::net::{TcpStream, ToSocketAddrs, Shutdown};
        use std::time::Duration;
        let addrs = address.to_socket_addrs().map_err(|e| format!("Bad TCP address: {e}"))?;
        let mut last_err = String::from("No TCP addresses resolved");
        for address in addrs.take(8) {
            match TcpStream::connect_timeout(&address, Duration::from_secs(3)) {
                Ok(mut stream) => {
                    stream.set_read_timeout(Some(Duration::from_secs(3)))
                        .map_err(|e| e.to_string())?;
                    stream.set_write_timeout(Some(Duration::from_secs(3)))
                        .map_err(|e| e.to_string())?;
                    stream.write_all(payload.as_bytes()).map_err(|e| e.to_string())?;
                    stream.shutdown(Shutdown::Write).map_err(|e| e.to_string())?;
                    let mut reply = Vec::new();
                    stream.take(64 * 1024 + 1).read_to_end(&mut reply)
                        .map_err(|e| e.to_string())?;
                    if reply.len() > 64 * 1024 { return Err("TCP reply exceeds 64 KiB".into()); }
                    let reply = String::from_utf8(reply).map_err(|_| "TCP reply is not UTF-8 text")?;
                    return Ok(BabelData::Text(reply));
                }
                Err(e) => last_err = format!("TCP connect failed: {e}"),
            }
        }
        Err(last_err)
    })();
    match run { Ok(value) => babel_result(true, value, ""), Err(err) => babel_result(false, BabelData::Null, &err) }
}
fn babel_http_serveonce(port: i64, response: String) -> BabelData {
    let run = (|| -> Result<BabelData, String> {
        if !babel_permission_allowed("network") { return Err("Babel capability denied: network".into()); }
        if !(1024..=65535).contains(&port) { return Err("HTTP server port must be 1024..65535".into()); }
        if response.len() > 128 * 1024 { return Err("HTTP server response exceeds 128 KiB".into()); }
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::time::{Duration,Instant};
        let listener = TcpListener::bind(("127.0.0.1", port as u16))
            .map_err(|e| format!("Cannot bind loopback HTTP port: {e}"))?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            match listener.accept() {
                Ok((mut connection,_)) => {
                    connection.set_read_timeout(Some(Duration::from_secs(3)))
                        .map_err(|e| e.to_string())?;
                    connection.set_write_timeout(Some(Duration::from_secs(3)))
                        .map_err(|e| e.to_string())?;
                    let mut request = Vec::<u8>::new();
                    let mut buffer = [0u8; 1024];
                    // Read only bounded HTTP headers. No request body support.
                    loop {
                        let size = connection.read(&mut buffer).map_err(|e| e.to_string())?;
                        if size == 0 { return Err("HTTP client disconnected".into()); }
                        request.extend_from_slice(&buffer[..size]);
                        if request.len() > 8192 { return Err("HTTP headers exceed 8 KiB".into()); }
                        if request.windows(4).any(|bytes| bytes == b"\r\n\r\n") { break; }
                    }
                    let request = std::str::from_utf8(&request)
                        .map_err(|_| "HTTP headers are not UTF-8 text")?;
                    let first = request.split("\r\n").next().ok_or("Missing HTTP request line")?;
                    let tokens: Vec<&str> = first.split(' ').collect();
                    if tokens.len() != 3 || !tokens[2].starts_with("HTTP/1.") ||
                        !matches!(tokens[0], "GET" | "HEAD") ||
                        !tokens[1].starts_with('/') || tokens[1].len() > 2048 ||
                        tokens[1].bytes().any(|b| b.is_ascii_control()) {
                        connection.write_all(
                            b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        ).map_err(|e| e.to_string())?;
                        return Err("Unsupported HTTP request".into());
                    }
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        response.len());
                    connection.write_all(header.as_bytes()).map_err(|e| e.to_string())?;
                    if tokens[0] != "HEAD" {
                        connection.write_all(response.as_bytes()).map_err(|e| e.to_string())?;
                    }
                    let mut info = std::collections::BTreeMap::new();
                    info.insert("method".into(), BabelData::Text(tokens[0].into()));
                    info.insert("path".into(), BabelData::Text(tokens[1].into()));
                    return Ok(BabelData::Object(info));
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= until { return Err("HTTP server accept timeout".into()); }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) => return Err(format!("HTTP accept failed: {e}")),
            }
        }
    })();
    match run { Ok(value) => babel_result(true, value, ""), Err(err) => babel_result(false, BabelData::Null, &err) }
}
