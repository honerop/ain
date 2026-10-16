# AIN

**An experimental, general-purpose programming language designed exclusively for AI-generated code, with an intentionally extensive built-in library.**

[![CI](https://github.com/honerop/ain/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/honerop/ain/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/implementation-Rust-orange.svg)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/status-experimental-yellow.svg)](#project-status)

AIN explores a simple question: **what would a general-purpose programming language look like if it were designed from the ground up for code written by AI models rather than humans?** AI-generated source code is AIN's **only intended use case**. General-purpose refers to the kinds of programs it can express, not to human authorship.

The source language uses compact, fixed-arity prefix expressions, automatic primitive type inference, and very little punctuation. The compiler experiments with native-code generation, a custom typed intermediate representation called **AIR**, and self-hosting.

> [!IMPORTANT]
> **Experimental / pre-alpha.** AIN is a research prototype, not a production-ready language. The LLVM backend supports only a subset of the language, compiler correctness and performance have not been comprehensively benchmarked, and CI is not currently green. Please do not deploy untrusted AIN programs.

## Design philosophy: batteries included (even if that means bloat)

AIN aims to be **general-purpose in capability but AI-only in authorship**: AI models should be able to generate ordinary applications, systems tools, automation, and other kinds of software in AIN. It is not a DSL limited to building AI agents; it is a language **for AI to write**.

The long-term goal is a **large, convenient standard library** with many common operations available directly: text and data handling, files, processes, networking, concurrency, and other everyday building blocks. AIN deliberately favors having these capabilities built in over making every program assemble a stack of dependencies. You could reasonably call that *bloated*. The tradeoff is intentional.

- **Optimized for generated code:** short, predictable syntax and ready-to-use operations reduce the tokens, imports, and scaffolding AI models need to generate.
- **General-purpose output:** AI models can generate programs in many domains, not merely chatbots or agents.
- **Intentionally extensive built-ins:** prefer a broad standard library even at the cost of some bloat, while keeping the compiler, runtime, and output binaries modular where practical.
- **Machine-friendly semantics:** deterministic, explicit behavior matters more than human-oriented syntax conventions, without weakening type checks or safety.

**Current reality:** AIN is still pre-alpha. Its built-in operations are limited, and many capabilities described here are **goals, not implemented features**.

## Functional-first programming foundation

Ain is functional-first, **not purely functional**: immutable-style data
transformations and callback combinators are the default, while explicit
mutable reference cells and controlled side effects remain available.

The native backend now has experimental structured `Data` values with lists,
records, tuple-style pairs, strict JSON parsing, canonical JSON output,
`map`, `filter`, `fold`, iteration and state cells. The compiler also
offers `ainc check-json` for machine-readable diagnostics.

See [the functional data guide](docs/functional-data.md). These are new
pre-alpha capabilities; more extensive static types, concurrent tasks,
database support and multimedia APIs remain future work.

## Native I/O, capability checks and asynchronous tasks

The native backend now exposes experimental file and environment operations,
HTTP GET/POST, explicit opt-in builtin capability checks, and asynchronous
Data tasks with `spawn`, `ready`, `poll` and `wait`.

See [the effects and tasks guide](docs/effects-tasks.md). Builtin permissions
are not an OS-level sandbox. More extensive networking, concurrency, database,
and multimedia APIs are planned, not yet complete.

## Serialization and structured storage

The native backend includes experimental prepared SQLite queries, CSV
parsing/writing, byte conversions, Base64, and optional zlib compression.
See the [SQLite guide](docs/sqlite.md) and
[serialization guide](docs/serialization.md).

These are incremental pieces of the proposed batteries-included library,
not evidence that all database engines, file formats, or multimedia APIs work.

## Statically typed functional collections

The native backend supports inferred homogeneous lists, nested lists, and
typed `listmap`, `listfilter`, `listfold` operations alongside dynamic
JSON `Data`. The compiler rejects mixed list elements and callback type
mismatches. See the [typed list guide](docs/typed-lists.md).

This is an incremental step toward generic collections, not full support for
user-defined types, closures, algebraic data types, or generic constraints.

## Typed optional values and recoverable errors

The Rust backend supports inferred `Option<T>` and `Result<T, String>`
operations, including type-checked fallback values and a recoverable
`tryread` filesystem function. See [Option and Result](docs/typed-option-result.md).

These builtins do not yet replace every panicking operation, and general
user-defined ADTs and pattern matching remain unimplemented.

## Structured HTTP client

The native backend supports `httprequest` for HTTP methods, validated
headers, request bodies, timeout/redirect settings, and structured status,
response headers and text. Transport errors are returned as recoverable
`Data` results. See the [HTTP client guide](docs/structured-http.md).

The API remains synchronous; true streaming and socket servers are future work.

## A first look

```text
add a b = + a b
square x = * x x

result = add square 3 square 4
print result
```

Output:

```text
25
```

There is no `fn` keyword, no call parentheses, and no declared integer types. The compiler discovers function arities from definitions and infers primitive types.

Unit-argument functions use `_`:

```text
greet _ = print "Hello from AIN"
greet _
```

For more examples, see [`examples/`](examples/) and the [language guide](docs/language.md).

## Quick start

**NixOS / Nix (recommended)**

```sh
git clone https://github.com/honerop/ain.git
cd ain
nix develop
cargo test
cargo run -- run examples/arithmetic.ain
```

**Other systems:** install Rust/Cargo and the native tools listed below, then run the same `cargo` commands from the repository root.

| Dependency | Used for |
| --- | --- |
| Rust / Cargo | Building the stage-zero compiler |
| `rustc` | Rust source backend, build and run |
| Clang with LLVM IR support | Experimental native LLVM backend |
| GCC | Self-hosted C bootstrap compiler |
| Python 3 | Independent stage-zero reference bootstrap |

Try the experimental direct LLVM path for supported numeric/boolean programs:

```sh
cargo run -- air examples/recursive.ain
cargo run -- llvm examples/recursive.ain
cargo run -- run-llvm examples/recursive.ain
```

The recursive example should print `720` and `42`.

### Compiler commands

| Command | Purpose |
| --- | --- |
| `cargo run -- check file.ain` | Parse and type-check an AIN program |
| `cargo run -- transpile file.ain` | Generate Rust source |
| `cargo run -- run file.ain` | Compile through Rust and execute |
| `cargo run -- build file.ain -o app` | Build a native executable through Rust |
| `cargo run -- air file.ain` | Inspect the experimental typed AIR |
| `cargo run -- llvm file.ain` | Generate LLVM IR directly |
| `cargo run -- run-llvm file.ain` | Build through Clang and execute |
| `cargo run -- build-llvm file.ain -o app` | Build through Clang with optimizations |

LLVM commands currently **reject unsupported operations**, including strings. They do not silently fall back to C.

## Architecture

```text
                         AIN source
                             |
                     Parser + type inference
                             |
                 +-----------+------------+
                 |                        |
       Rust code generator         Typed AIR (experimental)
                 |                        |
               rustc                 LLVM IR emitter
                 |                        |
          Native executable           Clang / LLVM
                                          |
                                   Native executable

                 Self-hosting experiment:
    bootstrap/compiler.ain -> C source -> GCC + small C runtime
```

AIN currently has **three distinct paths**:

| Path | Status | Notes |
| --- | --- | --- |
| Rust stage zero → Rust → native | Available / experimental | Primitive monomorphic type inference and more library operations |
| Typed AIR → LLVM IR → native | Experimental subset | Unboxed integers, booleans, units, recursion, checked arithmetic, conditionals |
| Self-hosted AIN → C → native | Bootstrap proof of concept | Compiler source is written in AIN, but uses a tagged-value C runtime |

**The production direction is typed AIR → LLVM IR → native code.** The C compiler is retained as a bootstrapping demonstration, not as the intended performance backend. The LLVM path is *not yet self-hosting*.

See [Architecture](docs/architecture.md) for backend boundaries and verification status.

## Self-hosting

The compiler in [`bootstrap/compiler.ain`](bootstrap/compiler.ain) can compile its own AIN source through successive stages.

```sh
./scripts/bootstrap.sh
```

This uses the Rust stage-zero compiler, then recompiles the AIN compiler through C and checks whether successive stages emit byte-identical C source.

A separate offline reference route uses Python, GCC, and the same AIN compiler source:

```sh
./scripts/bootstrap.sh --reference
```

The **reference route has been verified** with the included example programs. The default Rust-first bootstrap still needs independent verification in a working Rust CI environment. The current GitHub Actions history contains failures, so do not treat the CI badge as proof of a passing release.

## Project status

**Version:** 0.3.0 experimental source milestone (not a stable release).

**Implemented or partially implemented**

- Parenthesis-free, fixed-arity prefix expressions.
- Inferred primitive types and checked integer arithmetic.
- Function declarations without `fn`, with recursion and forward references.
- Basic string operations and printing in the Rust backend.
- Experimental structured Data, lists, records, JSON, functional combinators, explicit state cells, and structured compiler diagnostics.
- Experimental native filesystem/network functions, opt-in builtin permissions, and async Data tasks.
- Initial typed AIR with direct LLVM IR emission for supported primitives.
- Three-generation, C-based bootstrap experiment.

**Not yet available**

- Full-featured self-hosted AIR/LLVM compiler.
- Statically parameterized collections, user-defined record types, modules, generics, closures, and ownership analysis.
- Complete cross-backend semantic parity or extensive fuzzing.
- Proven performance advantage over Rust, C, or C++.
- A stable library/API, packages, or production security sandbox.

The immediate priority is **compiler correctness**, then feature parity and reproducible performance benchmarks. Runtime performance is the primary optimization goal; compilation time and AI token efficiency remain important.

## Repository layout

```text
bootstrap/           AIN-written self-hosted compiler
src/compiler.rs      Rust parser, type inference, Rust backend
src/compiler/air.rs  Experimental AIR + LLVM IR backend
runtime/             Tagged-value C runtime for the bootstrap
examples/            Small AIN programs
scripts/             Bootstrap scripts and independent reference compiler
tests/               Compiler / LLVM integration tests
docs/                Language and architecture documentation
.github/             CI and contributor templates
```

## Contributing

AIN is early-stage and changing quickly. Bug reports, minimal reproductions, test cases, and benchmark proposals are especially valuable.

Read [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md), and [the code of conduct](CODE_OF_CONDUCT.md) before contributing. Issues and pull requests are welcome; start with a small, testable change.

## License

[MIT](LICENSE). © 2026 AIN contributors.
