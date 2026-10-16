// zlib compression for Ain byte lists. Linked only when zpack/zunpack is used.
use std::ffi::c_ulong;
#[link(name="z")]
unsafe extern "C" {
    fn compressBound(source_len: c_ulong) -> c_ulong;
    fn compress2(destination: *mut u8, dest_len: *mut c_ulong,
        source: *const u8, source_len: c_ulong, level: i32) -> i32;
    fn uncompress(destination: *mut u8, dest_len: *mut c_ulong,
        source: *const u8, source_len: c_ulong) -> i32;
}
fn ain_zpack(value: AinData) -> AinData {
    let bytes = ain_bytes_checked(value).unwrap_or_else(|e| panic!("zpack: {e}"));
    let mut length = unsafe { compressBound(bytes.len() as c_ulong) };
    let mut result = vec![0u8; length as usize];
    let status = unsafe { compress2(result.as_mut_ptr(), &mut length,
        bytes.as_ptr(), bytes.len() as c_ulong, 6) };
    assert_eq!(status, 0, "zlib compress failed");
    result.truncate(length as usize);
    let mut header = (bytes.len() as u64).to_be_bytes().to_vec();
    header.extend(result);
    ain_bytes_data(header)
}
fn ain_zunpack(value: AinData) -> AinData {
    let result: Result<Vec<u8>, String> = (|| {
        let input = ain_bytes_checked(value)?;
        if input.len() < 8 { return Err("Compressed input is too short".into()); }
        let mut size = [0u8; 8];
        size.copy_from_slice(&input[..8]);
        let original = u64::from_be_bytes(size);
        if original > 8 * 1024 * 1024 { return Err("Decompressed output exceeds 8 MiB".into()); }
        let mut output = vec![0u8; original as usize];
        let mut actual = original as c_ulong;
        let status = unsafe { uncompress(output.as_mut_ptr(), &mut actual,
            input[8..].as_ptr(), (input.len() - 8) as c_ulong) };
        if status != 0 { return Err(format!("Corrupt zlib stream (code {status})")); }
        if actual != original as c_ulong { return Err("Decompressed length mismatch".into()); }
        Ok(output)
    })();
    match result {
        Ok(bytes) => ain_result(true, ain_bytes_data(bytes), ""),
        Err(e) => ain_result(false, AinData::Null, &e)
    }
}
