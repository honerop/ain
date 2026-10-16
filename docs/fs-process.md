# Filesystem, paths and subprocesses in Ain

Ain provides compact native operations for common filesystem and process tasks,
with explicit effects and recoverable errors.

```text
folder = pathjoin "reports" "2026"
print asbool field fsmkdir folder "ok"
files = fslist folder
print encode field files "value"
reply = processrun "printf" push array _ boxstr "Hello Ain"
print astext field field reply "value" "stdout"
```

## Filesystem operations

- `fslist PATH`: sorted list of entry filenames, as a recoverable Data result.
- `fsmkdir PATH`: recursively create a directory, return a Data result.
- `fsremove PATH`: delete one regular file, not recursively delete folders.
- `pathjoin LEFT RIGHT`: join native filesystem paths without I/O.
- `workingdir _`: current directory as a recoverable Data result.

`fslist` limits output to 10,000 entries and requires UTF-8 names.
`fslist` checks the optional `read` capability, while creation/removal
checks `write` when `AIN_RESTRICTED=1`.

## Subprocess execution

`processrun PROGRAM ARGS` accepts a Data list of text arguments. It starts
a subprocess **without invoking a shell**. The return value is a recoverable
Data record with `ok`, `value`, `error`; `value` contains `code`,
`stdout`, and `stderr`.

The process has a 10-second timeout, accepts at most 128 arguments, and
limits stdout/stderr to 1 MiB each. Binary output is rejected as a recoverable
error. A nonzero exit status is returned under `value.code`, not treated as a
transport failure.

The implementation checks `process` capability when restricted, but
**this is not a sandbox**: spawned processes run with the actual operating
system permissions and can have side effects independent of Ain's builtin
permission checks. For untrusted generated programs, use OS isolation.

The legacy `run PROGRAM ONE_ARG` convenience builtin remains available,
but the new `processrun` supports argument arrays, captured output and
recoverable error handling.

The native Rust backend implements these operations; the bootstrap C and
experimental AIR/LLVM subsets have not been upgraded. Generated-program
regression tests exercise real files, directories, processes and type errors.
