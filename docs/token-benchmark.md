# Ain token comparison benchmark

`ainc tokens` uses the real **o200k_base** BPE tokenizer (via Rust's optional
`tiktoken` crate). This reports source-token counts, not source bytes disguised
as tokens. The tokenizer is explicitly named because different model families
can split code differently.

Build once with the optional feature and run against the fixed benchmark corpus:

```sh
cargo run --features token-bench -- tokens benchmarks/tokens/*/* 
cargo run --features token-bench -- tokens benchmarks/tokens/*/* --json
```

Five deliberately small, equivalent-output tasks compare Ain against Python,
TypeScript, and Rust: arithmetic, factorial, string formatting, list mapping,
and polymorphic identity. Fixtures contain no comments. Results use the
**same tokenizer** on all languages. The output includes per-file bytes/tokens
and per-task Ain percentage savings against each other language. Negative
savings indicate Ain used more tokens.

This is a *microbenchmark*. Results depend on task choice, code formatting,
compiler/library availability, tokenizer, and how idiomatic each solution is.
It does **not** measure the success rate of generating correct programs, token
cost of model prompts/tool calls/retries, the runtime, or end-to-end efficiency.
Production conclusions require multiple models and a substantially larger
paired, correctness-checked task suite.

The default `cargo build` has no tokenizer dependency; only
`cargo build --features token-bench` downloads the optional crate. The native
`ainc bench` command remains available without this feature and benchmarks
compile/run time rather than text tokenization.
