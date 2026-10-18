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

## Initial measured results (18 October 2026)

Executed in [GitHub Actions Linux CI, run 39108070902](https://github.com/honerop/ain/actions/runs/39108070902)
using the exact fixture sources in `benchmarks/tokens/` and `o200k_base`:

| Task | Ain | Python | TypeScript | Rust |
|---|---:|---:|---:|---:|
| Arithmetic | 16 | 20 | 24 | 31 |
| Factorial | 23 | 28 | 33 | 43 |
| Generic identity | 19 | 25 | 31 | 38 |
| List mapping | 33 | 33 | 35 | 48 |
| String transform | 17 | 20 | 27 | 32 |
| **Total** | **108** | **126** | **150** | **192** |

Across these five tasks: Ain's source code used **14.3% fewer tokens** than
Python, **28.0% fewer** than TypeScript, and **43.8% fewer** than Rust.
The list-mapping example ties Python rather than beating it. These totals
describe only this very small corpus and are not a general language ranking.
They also do not demonstrate total model-token savings on generated programs.

The numbers above are copied from a successful CI invocation of
`ainc tokens --json`; for future code or tokenizer changes, rerun the
benchmark rather than trusting historical totals.

## Published machine-readable snapshot

The current o200k_base benchmark results are also published at
[`benchmarks/token-results.json`](../benchmarks/token-results.json).
The separate **Publish token benchmark data** workflow re-runs the native
Rust tokenizer whenever the corpus or tokenizer implementation changes,
and runs a weekly integrity check. It commits to `main` only if values changed.
Pull requests touching the benchmark data verify that the checked-in JSON
matches the code-generated output.

Other sites, including [honerop/personal](https://github.com/honerop/personal),
can fetch the public JSON from `main` and cache it. This is a published
snapshot that refreshes after source changes, **not real-time AI model
generation performance**, and it does not include model API token usage.

## Compact list syntax (18 October 2026)

The initial five-task baseline above used 33 Ain tokens for list mapping.
The compiler now supports readable list literals, optional short list
operation aliases, and unquoted callback names. The list benchmark retains
the same `xs` binding and result but now uses:

```ain
inc x = + x 1
xs = [1 2 3]
print ln lm inc xs
```

Measured with **the same `o200k_base` tokenizer**: 23 rather than 33 tokens
for the list task (30.3% fewer). The five-task Ain total decreases from
108 to **98 tokens**, against unchanged Python 126, TypeScript 150, Rust
192. On this limited corpus the updated source-token savings are **22.2%**
versus Python, **34.7%** versus TypeScript and **49.0%** versus Rust.

Alternative working variants are retained in `benchmarks/experiments`:
the original verbose 33-token code, an alias-only 26-token version, and
a 19-token version without the unused `xs` binding. We use 23 tokens for
the official comparison to keep the source structure aligned with the
existing other-language examples. This is a *source token count*, not proof
of end-to-end LLM token savings.
