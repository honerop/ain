# C ABI FFI in Ain

Ain has an experimental **Linux-native C ABI interoperability foundation**
limited to functions using signed 64-bit integers. In addition to the Rust
native backend, a system compiler and dynamic linker are required.

`ffi0 LIBRARY SYMBOL`, `ffi1 LIBRARY SYMBOL ARG`, and
`ffi2 LIBRARY SYMBOL ARG1 ARG2` call a trusted native C function declared
to return `int64` and accept zero, one, or two `int64` arguments.
All three return a recoverable `Data` record with fields `ok`, `value`,
`error`.

```text
value = ffi1 "libc.so.6" "labs" -7
print asbool field value "ok"
print asint field value "value"
```

The generated code uses `dlopen`, `dlsym`, and `dlclose`. Library load and
symbol resolution errors are recoverable. The builtin requires the `ffi`
permission when `AIN_RESTRICTED=1`. FFI dependencies are linked only in
generated programs that invoke FFI.

**Important safety limitation:** Native libraries are arbitrary native code,
and C symbol types cannot be verified through `dlsym`. Passing a function
with the wrong signature invokes undefined behavior and can crash the
process. The Ain builtin permission check does **not** provide OS process
isolation. Use only trusted C libraries and symbols whose signatures are
known precisely, or run the program inside a real sandbox.

This is **not yet a general FFI**: pointers, strings, C structs, floating-point
arguments, callbacks, variadic functions, ownership conventions and Windows
DLLs are not supported. All are tracked as future work.

End-to-end tests create a C shared library with 0-, 1- and 2-argument
functions, compile an Ain caller, verify actual results, and check missing
symbol/library and argument type errors.
