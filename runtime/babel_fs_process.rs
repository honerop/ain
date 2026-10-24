// Recoverable filesystem, path and process operations for Babel's native backend.
// Each effect checks Babel's optional permission allowlist. These checks are
// NOT a system-call sandbox, especially when spawning external programs.
fn babel_fs_list(path: String) -> BabelData {
    let run = (|| -> Result<BabelData, String> {
        if !babel_permission_allowed("read") { return Err("Babel capability denied: read".into()); }
        let mut names = Vec::<String>::new();
        for item in std::fs::read_dir(path).map_err(|e| e.to_string())? {
            let item = item.map_err(|e| e.to_string())?;
            names.push(item.file_name().into_string().map_err(|_| "Non-UTF-8 directory entry")?);
            if names.len() > 10_000 { return Err("Directory exceeds 10,000 entries".into()); }
        }
        names.sort();
        Ok(BabelData::List(names.into_iter().map(BabelData::Text).collect()))
    })();
    match run { Ok(v) => babel_result(true,v,""), Err(e) => babel_result(false,BabelData::Null,&e) }
}
fn babel_fs_mkdir(path: String) -> BabelData {
    let run = (|| -> Result<BabelData,String> {
        if !babel_permission_allowed("write") { return Err("Babel capability denied: write".into()); }
        std::fs::create_dir_all(path).map_err(|e| e.to_string())?;
        Ok(BabelData::Null)
    })();
    match run { Ok(v) => babel_result(true,v,""), Err(e) => babel_result(false,BabelData::Null,&e) }
}
fn babel_fs_remove(path: String) -> BabelData {
    let run = (|| -> Result<BabelData,String> {
        if !babel_permission_allowed("write") { return Err("Babel capability denied: write".into()); }
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
        Ok(BabelData::Null)
    })();
    match run { Ok(v) => babel_result(true,v,""), Err(e) => babel_result(false,BabelData::Null,&e) }
}
fn babel_path_join(base: String, segment: String) -> String {
    std::path::Path::new(&base).join(segment).to_string_lossy().into_owned()
}
fn babel_working_directory(_: ()) -> BabelData {
    match std::env::current_dir() {
        Ok(v) => babel_result(true,BabelData::Text(v.to_string_lossy().into_owned()),""),
        Err(e) => babel_result(false,BabelData::Null,&e.to_string())
    }
}
fn babel_process_run(program: String, argv: BabelData) -> BabelData {
    let run = (|| -> Result<BabelData,String> {
        if !babel_permission_allowed("process") { return Err("Babel capability denied: process".into()); }
        let BabelData::List(args) = argv else { return Err("Process arguments must be a Data list".into()); };
        if program.is_empty() || program.len() > 4096 || args.len() > 128 {
            return Err("Invalid program name or too many arguments".into());
        }
        let mut parsed = Vec::<String>::new();
        for arg in args {
            let BabelData::Text(s) = arg else { return Err("Process arguments must all be text".into()); };
            if s.len() > 32_768 || s.contains('\0') { return Err("Invalid process argument".into()); }
            parsed.push(s);
        }
        let id = format!("babel-child-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?.as_nanos());
        let out_path = std::env::temp_dir().join(format!("{id}.out"));
        let err_path = std::env::temp_dir().join(format!("{id}.err"));
        struct Cleanup(std::path::PathBuf,std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
                let _ = std::fs::remove_file(&self.1);
            }
        }
        let _cleanup = Cleanup(out_path.clone(),err_path.clone());
        let output_file = std::fs::OpenOptions::new().write(true).create_new(true)
            .open(&out_path).map_err(|e| e.to_string())?;
        let error_file = std::fs::OpenOptions::new().write(true).create_new(true)
            .open(&err_path).map_err(|e| e.to_string())?;
        let mut child = std::process::Command::new(program)
            .args(&parsed)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::from(output_file))
            .stderr(std::process::Stdio::from(error_file))
            .spawn().map_err(|e| format!("Could not start process: {e}"))?;
        // Keep short-running calls bounded by default, but allow explicit longer
        // operations such as offline video encoding from Babel applications.
        let timeout_ms = std::env::var("BABEL_PROCESS_TIMEOUT_MS").ok()
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|ms| (1..=1_800_000).contains(ms))
            .unwrap_or(10_000);
        let until = std::time::Instant::now()
            + std::time::Duration::from_millis(timeout_ms);
        let status = loop {
            match child.try_wait().map_err(|e| e.to_string())? {
                Some(status) => break status,
                None if std::time::Instant::now() >= until => {
                    child.kill().map_err(|e| e.to_string())?;
                    let _ = child.wait();
                    return Err(format!("Process timed out after {timeout_ms} milliseconds"));
                }
                None => std::thread::sleep(std::time::Duration::from_millis(10))
            }
        };
        let output = std::fs::read(&out_path).map_err(|e| e.to_string())?;
        let error = std::fs::read(&err_path).map_err(|e| e.to_string())?;
        if output.len() > 1024*1024 || error.len() > 1024*1024 {
            return Err("Process output exceeds 1 MiB".into());
        }
        let output = String::from_utf8(output).map_err(|_| "Process stdout is not UTF-8")?;
        let error = String::from_utf8(error).map_err(|_| "Process stderr is not UTF-8")?;
        let mut values = std::collections::BTreeMap::new();
        values.insert("code".into(),BabelData::Number(status.code().unwrap_or(-1).to_string()));
        values.insert("stdout".into(),BabelData::Text(output));
        values.insert("stderr".into(),BabelData::Text(error));
        Ok(BabelData::Object(values))
    })();
    match run { Ok(v) => babel_result(true,v,""), Err(e) => babel_result(false,BabelData::Null,&e) }
}
