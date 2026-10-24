# Effects, permissions, and native tasks

Babel is functional-first, but not purely functional. The default approach is
pure transformations on Data; effectful operations are explicit.

## Files and processes

- fsread PATH -> String: read a UTF-8 file.
- fswrite PATH STRING -> Unit: create or overwrite a file.
- fsexists PATH -> Bool: test whether a path exists.
- environ NAME -> String: read environment variable or return empty.
- run PROGRAM SINGLE_ARGUMENT -> Int: subprocess exit code (no shell).

## HTTP

- httpget URL -> String
- httppost URL BODY -> String (sends JSON content type)

Both operations call curl with HTTP(S)-only URL and redirect protocols and a
30-second request timeout. Only textual UTF-8 responses are supported; custom
headers, streaming, sockets, and a server runtime are not yet available.

## Optional capability checks

Run generated programs with BABEL_RESTRICTED=1 to enable builtin capability
checking. BABEL_PERMISSIONS is a comma-separated allowlist:

  BABEL_RESTRICTED=1 BABEL_PERMISSIONS=read,network ./program

Capabilities include read, write, env, process, network, ai, browser, desktop.
Set all to explicitly grant everything. With restrictions enabled, denied
builtin effects fail; without restrictions, effects are allowed as before for
backwards compatibility.

IMPORTANT: this is **not an operating-system sandbox**. It checks Babel's own
builtins only; it does not prevent arbitrary native code, unsafe FFI or called
external programs from using permissions granted by the OS. Never execute
untrusted code outside an actual sandbox.

The C bootstrap run builtin supports the same opt-in process permission
gate. New file and HTTP builtins are currently supported by the Rust backend.

## Asynchronous data tasks

Babel can now spawn a named Data -> Data callback in a native thread:

    double x = boxint * asint x 2
    task = spawn "double" boxint 21
    result = wait task
    print asint result

- spawn "callback" DATA -> Task
- ready TASK -> Bool
- poll TASK -> Data record with ok (ready), value, error
- wait TASK -> Data (blocks until complete)

Tasks are cloneable handles and cannot directly capture mutable reference
cells. A callback panic currently returns a tagged Data failure result.
Ainex still needs a nonblocking UI scheduling model to integrate tasks into
the main GTK4 event loop. Task support alone does not make every network or
GUI operation async.

## Testing

End-to-end tests compile and execute generated code, run an in-process
loopback HTTP server, verify file reads/writes, test task completion, and
reject denied permissions in a restricted process.

This API is experimental and subject to change.
