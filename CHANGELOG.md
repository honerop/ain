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
