# Native Babel benchmark

Run a benchmark from the Babel compiler itself, without Python or OpenCode.

```sh
cargo run -- bench examples/arithmetic.babel --runs 5
cargo run -- bench examples/recursive.babel --backend both --runs 5 --json
```

The command compiles native code through the selected backend and runs it
repeatedly. It reports compiler wall-clock time, median execution time,
executable size, and (when using both backends) checks matching stdout and
exit codes. Optional `--timeout-ms N` stops each program after the given duration.

`--backend rust` is the default because AIR/LLVM supports a smaller subset of Babel.
Use `--backend both` only on programs supported by both.

JSON includes `token_count: null` because source length and compiler timing
are **not** LLM token consumption. A separate experiment with access to
actual provider usage would be required to measure it.

**Important:** benchmarked source executes locally and is not sandboxed.
Only benchmark code you trust, or use real OS-level isolation.
