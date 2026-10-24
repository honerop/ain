# Babel

**An experimental, general-purpose programming language designed exclusively for AI-generated code, with an intentionally extensive built-in library.**

[![CI](https://github.com/honerop/babel/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/honerop/babel/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/implementation-Rust-orange.svg)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/status-experimental-yellow.svg)](#project-status)

Babel explores a simple question: **what would a general-purpose programming language look like if it were designed from the ground up for code written by AI models rather than humans?** AI-generated source code is Babel's **only intended use case**. General-purpose refers to the kinds of programs it can express, not to human authorship.

The source language uses compact, fixed-arity prefix expressions, automatic primitive type inference, and very little punctuation. The compiler experiments with native-code generation, a custom typed intermediate representation called **AIR**, and self-hosting.

> [!IMPORTANT]
> **Experimental / pre-alpha.** Babel is a research prototype, not a production-ready language. The LLVM backend supports only a subset of the language, compiler correctness and performance have not been comprehensively benchmarked, and CI is not currently green. Please do not deploy untrusted Babel programs.

## Design philosophy: batteries included (even if that means bloat)

Babel aims to be **general-purpose in capability but AI-only in authorship**: AI models should be able to generate ordinary applications, systems tools, automation, and other kinds of software in Babel. It is not a DSL limited to building AI agents; it is a language **for AI to write**.

The long-term goal is a **large, convenient standard library** with many common operations available directly: text and data handling, files, processes, networking, concurrency, and other everyday building blocks. Babel deliberately favors having these capabilities built in over making every program assemble a stack of dependencies. You could reasonably call that *bloated*. The tradeoff is intentional.

- **Optimized for generated code:** short, predictable syntax and ready-to-use operations reduce the tokens, imports, and scaffolding AI models need to generate.
- **General-purpose output:** AI models can generate programs in many domains, not merely chatbots or agents.
- **Intentionally extensive built-ins:** prefer a broad standard library even at the cost of some bloat, while keeping the compiler, runtime, and output binaries modular where practical.
- **Machine-friendly semantics:** deterministic, explicit behavior matters more than human-oriented syntax conventions, without weakening type checks or safety.

**Current reality:** Babel is still pre-alpha. Its built-in operations are limited, and many capabilities described here are **goals, not implemented features**.

## Real LLM code evaluation (OpenAI-compatible API)

`tools/llm_bench.py` can generate source with any OpenAI-compatible Chat Completions
API, or evaluate manually supplied responses. Set a base URL, model ID and an API
key environment variable; no OpenCode or third-party Python SDK is required.
It compares Babel, Python, Rust and TypeScript using real compile/run tests and
provider-reported tokens. See [LLM benchmark setup](docs/llm-benchmark.md).
Generated source must be executed in a properly isolated environment.

## Functional-first programming foundation

Babel is functional-first, **not purely functional**: immutable-style data
transformations and callback combinators are the default, while explicit
mutable reference cells and controlled side effects remain available.

The native backend now has experimental structured `Data` values with lists,
records, tuple-style pairs, strict JSON parsing, canonical JSON output,
`map`, `filter`, `fold`, iteration and state cells. The compiler also
offers `babelc check-json` for machine-readable diagnostics. User functions now
have experimental inferred parametric types in the Rust backend; LLVM explicitly
rejects polymorphic definitions until its specialization path is implemented.

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

## Composable GTK4 desktop apps

Babel's native GUI runtime now supports `guiupdate title layout` to render a
window without blocking and `guitick milliseconds` to pump GTK events while
background `spawn` tasks are running. Unlike repeatedly calling `gui`,
`guitick` preserves partially typed input and does not rebuild widgets.

Layouts now support nested rows, columns, panels, responsive wrapping grids,
CSS classes and custom GTK CSS rules. The GTK renderer also preserves scroll
position when conversations are redrawn. See [native layout documentation](docs/gui.md)
and [the GUI language reference](docs/language.md#responsive-gtk4-applications-in-babel).
An end-to-end native GTK4 test launches a window under Xvfb and verifies a timer
event. The GUI remains experimental. GTK4 supports Linux, macOS and Windows, but
Nix packaging is Linux-only and macOS/Windows GUI builds are not yet verified
in CI.

## Structured HTTP client

The native backend supports `httprequest` for HTTP methods, validated
headers, request bodies, timeout/redirect settings, and structured status,
response headers and text. Transport errors are returned as recoverable
`Data` results. See the [HTTP client guide](docs/structured-http.md).

The API remains synchronous; true streaming and socket servers are future work.

## Statically typed maps

In addition to lists and dynamic JSON records, the native backend supports
inferred `Map<String, T>` values with predictable sorted keys, persistent-style
updates, lookup and iteration-ready key/value collections. Invalid key types,
mixed values, and mismatched fallback types fail at compile time. See the
[typed maps guide](docs/typed-maps.md).

## Audio and graphics exporters

The native backend includes experimental `wavwrite` for synthesized PCM audio,
`svgwrite` for vector drawings, and `gltfwrite` for triangular 3D mesh assets.
All accept structured `Data` and return recoverable errors. They do not yet
provide streaming playback, interactive scenes, hardware-accelerated rendering,
or game physics. See the [multimedia guide](docs/media.md).

## Native TCP and local HTTP server

The standard runtime now provides `tcprequest` for bounded text TCP
request/response and `httpserveonce` for a loopback-only HTTP/1.1 response.
Both return recoverable Data results and enforce the optional network
capability. This is a networking foundation, not yet a general-purpose
streaming socket or production server framework.

See the [network reference](docs/network.md).

## Native C ABI interoperability

The Linux-native backend supports experimental `ffi0`, `ffi1`, and `ffi2`
calls to trusted C functions with 64-bit integer signatures, with recoverable
library/symbol errors. General pointer/struct/callback ABI support and safe
native-code sandboxing are **not** implemented yet. See the
[FFI guide](docs/ffi.md) for explicit undefined-behavior warnings.

## Native TOML configuration serialization

The standard runtime now includes `tomlparse` and `tomlwrite` for simple
TOML configuration files, with recoverable errors, sorted keys and nested
tables. It intentionally rejects unsupported TOML 1.0 constructs such as
array-of-tables and multiline strings. See the [TOML guide](docs/toml.md).

## Deterministic formatting for AI-generated source

`babelc fmt FILE.babel` emits canonical spacing without changing program
semantics; `babelc fmt-check FILE.babel` fails in CI when source is not
canonically formatted. Comments, blank lines, literal string contents,
and definition order remain intact. See the [formatter guide](docs/formatter.md).

## Heterogeneous typed tuples

The native backend supports statically inferred two-element tuples with
`tuple2`, `tuplefst`, `tuplesnd` and `tupleswap`, so AI-generated
programs can combine an integer and a string without boxing into dynamic
JSON values. See the [typed tuples guide](docs/typed-tuples.md).
User-defined structs and arbitrary-length tuples remain future work.

## Recoverable filesystem and subprocess APIs

Native programs can list directories, create paths, remove individual files,
join paths, inspect the working directory, and run processes using structured
argument arrays with captured output and errors. The operations are guarded
by Babel's optional builtin permission checks, **not** by an OS sandbox.
See the [filesystem and process guide](docs/fs-process.md).

## AI-oriented diagnostics and effect checks

`babelc check-json` emits machine-readable error categories and targeted
repair hints; `babelc run-checked` defaults Babel **builtins** to denied effects,
with explicit opt-in via `BABEL_PERMISSIONS=read,network`, for example.

These builtin checks are **not an OS sandbox** and do not protect against
untrusted native FFI or permitted subprocesses. See the
[AI tooling guide](docs/ai-tooling.md).

## Concise functional transformations

The standard library now includes typed text/list operations such as
`split`, `strjoin`, `lines`, `listreverse`, `listtake`, `listdrop`,
`listappend`, `listflatten`, `sortint`, and `sortstr`. They compile
directly to efficient native operations and reject incompatible element
types. See the [functional standard-library guide](docs/functional-stdlib.md).

## Free OpenCode model provider

`chat "opencode" "" PROMPT` or `chat "opencode" "" PROMPT` calls an
installed OpenCode CLI with the free `opencode/mimo-v2.6-flash-free` model
by default and parses its JSONL text events. OpenBot itself is a desktop
workspace, not a model API. See [OpenCode integration](docs/opencode.md).

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
greet _ = print "Hello from Babel"
greet _
```

For more examples, see [`examples/`](examples/) and the [language guide](docs/language.md).

## Download prebuilt compiler binaries

Every push to `main` and every pull request builds release versions of
`babelc` for Linux x86-64, Linux ARM64, macOS Intel/Apple Silicon and Windows
x86-64. Choose a recent successful run under
[GitHub Actions > Build babelc binaries](https://github.com/honerop/babel/actions/workflows/build-binaries.yml)
and download the `babelc-<platform>` artifact. Each archive includes the binary,
license, README, and SHA-256 checksum. Artifacts expire after 30 days.

Linux archives use statically linked **musl** binaries, so the compiler itself
does not require the usual glibc dynamic loader on NixOS. However, building
Babel programs still calls external `rustc` (or Clang for the LLVM subset)
and generated applications may need their own native system libraries.
The compiler is not a standalone installer for those toolchains.

## Quick start

**NixOS / Nix (recommended)**

```sh
git clone https://github.com/honerop/babel.git
cd babel
nix develop
cargo test
cargo run -- run examples/arithmetic.babel
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
cargo run -- air examples/recursive.babel
cargo run -- llvm examples/recursive.babel
cargo run -- run-llvm examples/recursive.babel
```

The recursive example should print `720` and `42`.

### Compiler commands

| Command | Purpose |
| --- | --- |
| `cargo run -- check file.babel` | Parse and type-check an Babel program |
| `cargo run -- transpile file.babel` | Generate Rust source |
| `cargo run -- run file.babel` | Compile through Rust and execute |
| `cargo run -- build file.babel -o app` | Build a native executable through Rust |
| `cargo run -- air file.babel` | Inspect the experimental typed AIR |
| `cargo run -- llvm file.babel` | Generate LLVM IR directly |
| `cargo run -- run-llvm file.babel` | Build through Clang and execute |
| `cargo run -- build-llvm file.babel -o app` | Build through Clang with optimizations |

LLVM commands currently **reject unsupported operations**, including strings. They do not silently fall back to C.

## Architecture

```text
                         Babel source
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
    bootstrap/compiler.babel -> C source -> GCC + small C runtime
```

Babel currently has **three distinct paths**:

| Path | Status | Notes |
| --- | --- | --- |
| Rust stage zero → Rust → native | Available / experimental | Primitive monomorphic type inference and more library operations |
| Typed AIR → LLVM IR → native | Experimental subset | Unboxed integers, booleans, units, recursion, checked arithmetic, conditionals |
| Self-hosted Babel → C → native | Bootstrap proof of concept | Compiler source is written in Babel, but uses a tagged-value C runtime |

**The production direction is typed AIR → LLVM IR → native code.** The C compiler is retained as a bootstrapping demonstration, not as the intended performance backend. The LLVM path is *not yet self-hosting*.

See [Architecture](docs/architecture.md) for backend boundaries and verification status.

## Self-hosting

The compiler in [`bootstrap/compiler.babel`](bootstrap/compiler.babel) can compile its own Babel source through successive stages.

```sh
./scripts/bootstrap.sh
```

This uses the Rust stage-zero compiler, then recompiles the Babel compiler through C and checks whether successive stages emit byte-identical C source.

A separate offline reference route uses Python, GCC, and the same Babel compiler source:

```sh
./scripts/bootstrap.sh --reference
```

The **reference route has been verified** with the included example programs. The default Rust-first bootstrap still needs independent verification in a working Rust CI environment. Check the latest CI runs and release build results before relying on a binary.

## Project status

**Version:** 0.3.1 experimental source milestone (not a stable release).

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
bootstrap/           Babel-written self-hosted compiler
src/compiler.rs      Rust parser, type inference, Rust backend
src/compiler/air.rs  Experimental AIR + LLVM IR backend
runtime/             Tagged-value C runtime for the bootstrap
examples/            Small Babel programs
scripts/             Bootstrap scripts and independent reference compiler
tests/               Compiler / LLVM integration tests
docs/                Language and architecture documentation
.github/             CI and contributor templates
```

## Contributing

Babel is early-stage and changing quickly. Bug reports, minimal reproductions, test cases, and benchmark proposals are especially valuable.

Read [CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md), and [the code of conduct](CODE_OF_CONDUCT.md) before contributing. Issues and pull requests are welcome; start with a small, testable change.

## License

[MIT](LICENSE). © 2026 Babel contributors.
