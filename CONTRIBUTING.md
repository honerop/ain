# Contributing to AIN

Thanks for helping improve AIN. It is a pre-alpha compiler experiment: small, well-tested contributions are more useful than broad API rewrites.

## Before opening an issue

- Check whether the problem occurs on the current `main` branch.
- Include your operating system, toolchain versions and the selected backend.
- Reduce compiler bugs to a minimal `.ain` reproduction.
- Avoid publishing tokens, credentials, private data, or potentially exploitable details in an issue. Follow [SECURITY.md](SECURITY.md) for vulnerabilities.

## Development setup

```sh
git clone https://github.com/honerop/ain.git
cd ain
nix develop             # optional, on systems with Nix
cargo test
```

For a supported program, compare the backends:

```sh
cargo run -- run examples/arithmetic.ain
cargo run -- run-llvm examples/arithmetic.ain
```

To validate the C self-hosting experiment:

```sh
./scripts/bootstrap.sh --reference
./scripts/bootstrap.sh
```

The second route requires the Rust stage-zero compiler; document what you actually ran.

## Pull requests

1. Describe the observable problem and approach in the PR.
2. Include tests for success, invalid input, and relevant boundary cases.
3. Preserve checked overflow and short-circuit behavior across backends.
4. For builtin changes, update the lexer/arity registry, inference, Rust backend, AIR/LLVM support where applicable, self-hosted compiler, and runtime, or document unsupported paths.
5. Run `cargo fmt --check` and `cargo test` if available. Do not claim checks passed without executing them.
6. Keep PRs focused, disclose any known failures, and explain whether performance improvements are measured or hypothetical.

Code in `src/compiler/air.rs` is experimental. Don't weaken diagnostics or introduce silent C fallbacks just to make programs compile.

See [Architecture](docs/architecture.md) and [Language guide](docs/language.md) for context.
