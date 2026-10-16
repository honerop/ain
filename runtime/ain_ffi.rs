// Deliberately narrow native C ABI FFI for signed 64-bit integer functions.
// The caller MUST know the library symbol has the declared ABI/signature.
// Loading arbitrary native libraries is inherently unsafe: Ain's builtin
// permission gate is NOT a native-code sandbox.
#[link(name="dl")]
unsafe extern "C" {
    fn dlopen(path: *const std::ffi::c_char, flags: i32) -> *mut std::ffi::c_void;
    fn dlsym(handle: *mut std::ffi::c_void, symbol: *const std::ffi::c_char) -> *mut std::ffi::c_void;
    fn dlclose(handle: *mut std::ffi::c_void) -> i32;
    fn dlerror() -> *const std::ffi::c_char;
}
struct AinForeignLibrary(*mut std::ffi::c_void);
impl Drop for AinForeignLibrary {
    fn drop(&mut self) { unsafe { dlclose(self.0); } }
}
fn ain_foreign_error() -> String {
    let ptr = unsafe { dlerror() };
    if ptr.is_null() { "Unknown native library error".into() }
    else { unsafe { std::ffi::CStr::from_ptr(ptr) }.to_string_lossy().into_owned() }
}
fn ain_ffi_call(lib: String, function: String, args: &[i64]) -> AinData {
    let run = (|| -> Result<AinData, String> {
        if !ain_permission_allowed("ffi") { return Err("Ain capability denied: ffi".into()); }
        if lib.is_empty() || lib.len() > 1024 || function.is_empty() || function.len() > 128
            || !function.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err("Invalid native library or function name".into());
        }
        let path = std::ffi::CString::new(lib).map_err(|_| "Library path contains NUL")?;
        let symbol = std::ffi::CString::new(function).map_err(|_| "Symbol contains NUL")?;
        // RTLD_NOW=2; do not use RTLD_GLOBAL or dlopen(NULL).
        let ptr = unsafe { dlopen(path.as_ptr(), 2) };
        if ptr.is_null() { return Err(format!("Cannot load library: {}", ain_foreign_error())); }
        let handle = AinForeignLibrary(ptr);
        // Clear any previous loader error before symbol lookup.
        unsafe { dlerror(); }
        let function = unsafe { dlsym(handle.0, symbol.as_ptr()) };
        let error = unsafe { dlerror() };
        if !error.is_null() || function.is_null() {
            return Err(format!("Cannot resolve C symbol: {}",
                if error.is_null() { "not found".into() }
                else { unsafe { std::ffi::CStr::from_ptr(error) }.to_string_lossy().into_owned() }));
        }
        // SAFETY: The symbol's actual signature cannot be checked at runtime.
        // This operation is restricted to explicit i64 signatures. Call trusted
        // symbols only. A mismatched signature can cause undefined behavior.
        let value: i64 = unsafe {
            match args {
                [] => {
                    let function: unsafe extern "C" fn() -> i64 = std::mem::transmute(function);
                    function()
                }
                [a] => {
                    let function: unsafe extern "C" fn(i64) -> i64 = std::mem::transmute(function);
                    function(*a)
                }
                [a,b] => {
                    let function: unsafe extern "C" fn(i64,i64) -> i64 = std::mem::transmute(function);
                    function(*a,*b)
                }
                _ => return Err("Only 0..2 integer arguments are supported".into())
            }
        };
        Ok(AinData::Number(value.to_string()))
    })();
    match run { Ok(value) => ain_result(true, value, ""), Err(e) => ain_result(false, AinData::Null, &e) }
}
fn ain_ffi0(lib: String, function: String) -> AinData { ain_ffi_call(lib,function,&[]) }
fn ain_ffi1(lib: String, function: String, a: i64) -> AinData { ain_ffi_call(lib,function,&[a]) }
fn ain_ffi2(lib: String, function: String, a: i64, b: i64) -> AinData { ain_ffi_call(lib,function,&[a,b]) }
