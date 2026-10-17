# Machine-friendly diagnostics and checked execution

Ain is designed exclusively for AI-authored source code, so tooling should
support automated compile/repair loops.

## Compiler diagnostics

`ainc check-json FILE.ain` returns a stable structured JSON object.
Errors contain a `severity`, `line`, `code`, `message`, and `hint`.

```json
{"ok":false,"diagnostics":[{"severity":"error","code":"E_TYPE","line":1,"message":"type mismatch: ...","hint":"Check the expected argument and return types"}]}
```

Current diagnostic classes are `E_TYPE`, `E_UNKNOWN_VARIABLE`,
`E_UNKNOWN_FUNCTION`, `E_INFERENCE`, `E_EXPRESSION`,
`E_UNTERMINATED`, `E_DUPLICATE`, and `E_COMPILER`.
Classes are broad: the underlying messages remain the authoritative compiler
diagnostics. Precise column ranges, related locations, and automated code fixes
are not yet implemented.

## Default-deny builtin execution

`ainc run-checked FILE.ain` compiles and runs a program with Ain builtin
effects restricted. With no `AIN_PERMISSIONS`, builtins for filesystem
access, environment access, processes, network, browser, AI providers and
desktop GUI are denied.

Grant only the capabilities needed by the generated program:

```sh
AIN_PERMISSIONS=read,network ainc run-checked app.ain
```

`ainc run` remains the backward-compatible unguarded mode.
`run-checked` sets `AIN_RESTRICTED=1` only for the executed program, not
for the compiler.

**Critical limitation:** this is builtin **permission checking**, not an
operating-system sandbox. It cannot confine native FFI or any subprocess
the program is allowed to spawn, and it is not a security boundary against
malicious native code. For code that you do not trust, execute in a real
isolated environment with filesystem, process and network restrictions.

Tests verify default-deny file reading, explicit permission grants, and
structured diagnostic categories. These are incremental compiler/tooling
capabilities, not a substitute for static effect typing or deterministic
machine-verifiable proofs.

## Deterministic runtime assertions

`assert CONDITION` takes a Boolean expression and returns unit on success.
If false, the program stops with an assertion failure. It is supported by
both the Rust and LLVM backends, including after optimization.

```text
assert == + 2 3 5
```

These are runtime checks, **not proofs**. To verify a property for all inputs,
use an external verification system until Ain has a sound proof checker.
