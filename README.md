# AIN compiler v0.2.0: self-hosting bootstrap

AIN is an experimental, compact, prefix programming language built for AI-written code. Its source syntax removes `fn`, function-call parentheses, return statements and explicit types when inference suffices.

**v0.2.0 adds a self-hosted compiler written in AIN** (`bootstrap/compiler.ain`). It reads AIN source from standard input and emits C source against `runtime/ain_runtime.h`. The original compiler written in Rust remains the **stage-zero compiler**: it typechecks AIN and emits Rust to be compiled with `rustc`.

This is an experimental bootstrapping milestone, not yet a fully production-ready compiler. The self-hosted C backend currently supports a bootstrap subset of the Rust compiler's features, performs only minimal validation, and uses a tagged-value runtime instead of statically specialized C types. It does *not* yet reproduce the Rust compiler's full static type checking or optimization.

## Syntax

```text
add a b = + a b
square x = * x x
print add square 3 square 4
say _ = print "Hello from AIN"
say _
```

`_` is a unit argument. All calls use fixed-arity prefix syntax. `if`, `and`, and `or` evaluate only the required branch.

## Build and bootstrap on NixOS

```sh
nix develop
cargo test
./scripts/bootstrap.sh
```

Or with Rust, GCC, and Python installed without Nix:

```sh
cargo test
./scripts/bootstrap.sh
```

`bootstrap.sh` executes these stages:

1. **Stage 0 (Rust)**: compile `bootstrap/compiler.ain` using the Rust AIN compiler (`cargo run -- build ...`). The result is a native executable of the AIN-written compiler, implemented by the Rust-generated source.
2. **Stage 1 (AIN → C)**: run that binary on `bootstrap/compiler.ain`, emitting `build/stage1.c`. GCC builds `build/stage1` using the included runtime.
3. **Stage 2 (AIN → C)**: run stage 1 on the same `.ain` source, generating `build/stage2.c`. Build `build/stage2` with GCC.
4. **Stage 3**: run stage 2 on `bootstrap/compiler.ain` to generate `build/stage3.c`.
5. Assert `stage1.c == stage2.c == stage3.c` byte-for-byte, then compile and execute sample programs through the stage-two compiler.

All stages after stage zero compile AIN using a compiler **written in AIN**. A C compiler and small trusted runtime remain necessary. The C generated at each stage is checked for a fixed point; binary bytes are not expected to match because host compiler flags or environments may vary.

### Offline GCC/reference test without Rust

```sh
./scripts/bootstrap.sh --reference
```

This uses `scripts/stage0_reference.py`, an independent Python stage-zero AIN subset compiler, rather than the Rust stage-zero compiler. **This mode actually verifies the stage-one/stage-two/stage-three self-hosting cycle**, but does *not* verify that the Rust stage-zero compiler accepts and builds the bootstrap source. Use the default command for that stronger end-to-end check.

### Compile another AIN file using the self-hosted compiler

```sh
./build/stage2 < examples/recursive.ain > build/example.c
gcc -std=gnu11 -O2 -I runtime build/example.c -o build/example
./build/example
```

Expected output: `720` followed by `42`.

## v0.2.0 builtins added to Rust stage zero

| Builtin | Signature | Purpose |
|---|---|---|
| `input _` | unit → string | Read all standard input |
| `word s` | string → string | First raw lexical token, respecting string quotes and comments |
| `skip s` | string → string | Remainder after the first raw lexical token |
| `before s sep` | string × string → string | Prefix before the first occurrence (whole string if absent) |
| `after s sep` | string × string → string | Suffix after the first occurrence (empty if absent) |
| `num s` | string → integer | Parse signed 64-bit integer |
| `isnum s` | string → boolean | Test for a signed 64-bit integer literal |

`input`, `word`, and `skip` make it possible to implement the signature pass, recursive prefix parser, and C emitter in AIN. They also become part of the trusted C bootstrap runtime. The stage-one compiler handles name/arity discovery, function forward declarations, expression parsing, and C code generation in AIN source.

## Project files

```text
bootstrap/compiler.ain          Self-hosted AIN -> C compiler (AIN source)
runtime/ain_runtime.h          Small tagged-value runtime used by generated C
src/compiler.rs               Rust-based stage-zero compiler (AIN -> Rust)
src/main.rs                   Stage-zero CLI (check, transpile, build, run)
scripts/bootstrap.sh          End-to-end bootstrap and fixed-point verification
scripts/stage0_reference.py    Independent offline stage-zero checker/compiler
examples/                     Programs compiled by the self-hosted compiler
tests/                        Rust compiler tests
```

## Constraints and limitations

- The self-hosted compiler covers the grammar and builtins required to compile itself and the included examples. It is not a full replacement for the stage-zero Rust compiler yet.
- The self-hosted C path uses tagged dynamic values and allocates temporary strings until the process exits. This is adequate for demonstration, not a memory-efficient production runtime.
- C runtime string case conversion is ASCII-oriented and does not exactly match Rust's Unicode behavior.
- The self-hosted compiler does not implement stage-zero's static type inference, complete identifier validation, or line-numbered diagnostics. GCC may reject source that Rust stage zero would reject earlier.
- Native compilation requires GCC (C11 with GNU extensions for checked arithmetic) or a compatible compiler.
- Generated C is **not sandboxed**. Do not compile or execute untrusted AIN programs.
- No user-defined records, generic types, modules, closures, automatic concurrency, macros, or incremental AST edits.
- The self-hosted compiler reads from stdin and writes generated C to stdout; there is no native CLI argument parser in AIN yet.
- The development environment used for this release has GCC and Python, but no Rust toolchain or network access to install one. The **offline reference bootstrap was executed successfully**, including all three matching generated C stages and the sample programs. The default Rust-first bootstrap has not been run in this environment.

## Next milestones

A self-hosted type checker, unified optimized IR/backends, improved lexical/token parsing, stable module system, diagnostics, and integrated performance/token-count benchmarks. MIT license.

## Experimental AIR and LLVM backend (0.3.0)

AIN now has an initial **AIR (AIN Intermediate Representation)** typed IR and a direct LLVM backend for numeric/boolean programs. It lowers AIN expressions to concrete SSA values (`i64`, `i1`, `i8`) and emits LLVM IR, **not C source**. Checked arithmetic invokes LLVM overflow intrinsics and traps on overflow rather than silently changing arithmetic semantics.

```bash
nix develop
cargo test
cargo run -- air examples/recursive.ain
cargo run -- llvm examples/recursive.ain -o build/recursive.ll
cargo run -- run-llvm examples/recursive.ain
cargo run -- build-llvm examples/arithmetic.ain -o build/arithmetic
```

AIR performs small safe folds before LLVM optimization. Clang `-O3` performs the machine-code optimization and linking. The compiler currently emits LLVM IR text directly, rather than calling the LLVM APIs through a binding.

**Supported in this backend:** fixed-arity user functions, recursion, integer/boolean/unit values, top-level values, checked `+ - * / %`, `neg`, `abs`, `pow`, `min`, `max`, `clamp`, comparisons, lazy `if`, `and`, `or`, and `print` of primitive values. The self-hosted `bootstrap/compiler.ain` and string/file operations still require the original compiler and C bootstrap path. A compiler error, not a fallback to C, is emitted for unsupported LLVM features.

The LLVM pipeline is an **experimental preview**. It is not self-hosted and not evidence of faster performance than optimized Rust or C. Benchmark runtime and compile time with matched semantics and equivalent optimization settings before claiming performance improvements.
