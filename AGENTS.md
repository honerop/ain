# AIN development guide

AIN syntax deliberately omits `fn`, parentheses, braces and explicit type annotations for normal source programs. Expressions are fixed-arity prefix trees and statements end at the source line boundary.

This repository has **two compiler implementations**:
1. Rust stage zero in `src/compiler.rs`, which has parsing, primitive monomorphic type inference and Rust code generation.
2. AIN stage one in `bootstrap/compiler.ain`, which parses and emits C; `runtime/ain_runtime.h` defines generated C's primitive operations.

To test bootstrapping, run `./scripts/bootstrap.sh` in a Rust/GCC shell. If Rust is unavailable, `./scripts/bootstrap.sh --reference` proves self-hosting for the independent stage-zero route but does not validate Rust compilation. Do not misstate that distinction.

Change builtin definitions consistently across Rust `builtin_arity`, Rust type inference, Rust codegen, AIN `base _`, AIN `sym`, and the C runtime. New syntax or builtins must include tests for recursion, nesting, error cases and a three-generation fixed-point check. Keep runtime permissions minimal and never execute generated C from untrusted input.
