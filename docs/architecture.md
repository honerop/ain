# Compiler architecture

AIN is a language experiment with several backend paths. They do not yet implement the same feature set.

## Rust stage-zero backend

```text
AIN source
  -> lexer and fixed-arity parser
  -> primitive type inference and checking
  -> Rust source generator
  -> rustc
  -> executable
```

Implemented mainly in [`src/compiler.rs`](../src/compiler.rs), with the CLI in [`src/main.rs`](../src/main.rs). This is the compatibility backend and has broader string support.

## AIR and LLVM prototype

```text
AIN source
  -> frontend / type inference
  -> AIR typed expressions
  -> safe constant folding
  -> textual LLVM IR
  -> Clang -O3 / LLVM
  -> executable
```

AIR lives in [`src/compiler/air.rs`](../src/compiler/air.rs). It currently supports unboxed i64, i1 and unit representations, primitive arithmetic, comparisons, lazy conditionals, recursion and printing.

The goal is **native performance with correctly preserved semantics**. Direct LLVM IR generation is not automatically faster than optimized C/Rust; actual performance claims require equivalent benchmarks.

Strings, user collections, modules and the self-hosted compiler are *not* yet supported by the AIR pipeline. Unsupported operations fail explicitly.

## Self-hosting experiment

```text
AIN-written compiler (bootstrap/compiler.ain)
    -> stage-zero compiler
    -> native stage-one compiler
    -> compile the same AIN source to C
    -> GCC + runtime/ain_runtime.h
    -> next stage and fixed-point checks
```

The AIN-written compiler emits C and uses tagged runtime values. That is a **bootstrap proof of concept**, not the production backend or an optimization victory.

- `./scripts/bootstrap.sh --reference`: independently tested using Python/GCC; three generated C stages matched and bundled examples worked.
- `./scripts/bootstrap.sh`: Rust-first route provided, but not yet verified in a successful GitHub CI run.

## Correctness and performance priorities

1. Preserve typed source semantics, short-circuiting and overflow behavior.
2. Ensure all backends agree on supported behavior.
3. Specialize values and eliminate unnecessary runtime allocations.
4. Benchmark runtime first, compile time second, using real programs.
5. Move the AIN-written compiler onto AIR and native code only after required language primitives are supported.

The current `CI` workflow is not known to pass. A failed badge must not be represented as proof of passing builds.

## Security boundary

Compilation and execution are *not* sandboxed. Both generated executables and build scripts can invoke native compilers and operating-system facilities. Do not run untrusted AIN programs outside a properly isolated environment.
