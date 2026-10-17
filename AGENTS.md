# AIN development guide

AIN syntax deliberately omits `fn`, parentheses, braces and explicit type annotations for normal source programs. Expressions are fixed-arity prefix trees and statements end at the source line boundary.

This repository has **two compiler implementations**:
1. Rust stage zero in `src/compiler.rs`, which has parsing, inferred rank-1 parametric function types with monomorphic recursive SCCs and Rust code generation.
2. AIN stage one in `bootstrap/compiler.ain`, which parses and emits C; `runtime/ain_runtime.h` defines generated C's primitive operations.

To test bootstrapping, run `./scripts/bootstrap.sh` in a Rust/GCC shell. If Rust is unavailable, `./scripts/bootstrap.sh --reference` proves self-hosting for the independent stage-zero route but does not validate Rust compilation. Do not misstate that distinction.

Change builtin definitions consistently across Rust `builtin_arity`, Rust type inference, Rust codegen, AIN `base _`, AIN `sym`, and the C runtime. New syntax or builtins must include tests for recursion, nesting, error cases and a three-generation fixed-point check. Keep runtime permissions minimal and never execute generated C from untrusted input.

## v0.3 AIR / LLVM work

The new experimental `src/compiler/air.rs` consumes a checked AST and emits unboxed LLVM IR. Preserve checked arithmetic and short-circuit semantics. Keep all AIR opcodes explicitly typed; unsupported syntax must fail with a compiler diagnostic rather than silently use C or an unsafe runtime. Run `cargo test` and `cargo run -- run-llvm examples/recursive.ain` with Clang installed before merging any AIR changes. The C bootstrap remains intact until the self-hosted compiler can consume AIR and produce native code on its own.

The general-purpose `run` builtin executes an external program with exactly one argument, no shell. Keep application-specific integrations (GTK, browser automation, model providers) out of the compiler.

GUI, chat and browser are reusable AIN builtins, implemented inside `runtime/` and made available via Rust backend. Applications should express layout and interactions in AIN. Preserve portable C bootstrap, and explicitly reject unsupported features on AIR/LLVM.
