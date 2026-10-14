# Changelog

## 0.2.0 — bootstrapping (2026-10-13)

- Implemented `bootstrap/compiler.ain`, a self-hosting AIN-to-C compiler using AIN's prefix syntax, inferred primitive types at stage zero, and underscore/unit function calls.
- Added input and lexical string builtins (`input`, `word`, `skip`, `before`, `after`, `num`, `isnum`) to the Rust stage-zero compiler.
- Added the tagged-value C runtime, three-stage fixed-point verification, and independent Python stage-zero reference for verification without Rust.
- Verified the reference bootstrap cycle on GCC: stages 1/2/3 produce byte-identical C, and the arithmetic, string, conditional, unit-function, and recursive examples execute as expected.
- Added the bootstrap script to CI and expanded Nix dev shell to include GCC/Python.
- Known: Rust-based stage-zero bootstrap cannot be runtime-tested in this packaging environment; self-hosted C currently lacks stage-zero's complete static checking and shares a trusted C runtime.

## 0.1.0 — initial Rust compiler

- Initial Rust lexer, prefix parser, primitive type inference, Rust backend, CLI, and Nix shell.
- Supported one-line definitions, unit argument, fixed-arity builtins and example programs.

## 0.3.0 — experimental AIR / LLVM backend (2026-10-14)

- Added a typed AIN Intermediate Representation (AIR) with concrete primitive values and safe constant folding.
- Added direct LLVM IR generation for i64/bool/unit expressions, functions, recursion, lazy conditionals, comparisons, checked arithmetic and printing, without C source generation or the tagged C runtime.
- Added `ainc air`, `ainc llvm`, `ainc build-llvm` and `ainc run-llvm` to inspect or execute the LLVM path via Clang `-O3`.
- Added LLVM native executable tests and explicit rejection of unsupported operations such as strings rather than generating incorrect code.
- Retained the Rust transpiler and existing C self-hosting bootstrap for compatibility. AIR is an experimental numeric subset, **not** a complete replacement or self-hosted yet.
- Compiler infrastructure uses Clang for LLVM IR code generation; does not yet implement PGO, module LTO, vectors, collections, advanced alias analysis or a self-hosted AIR backend.
