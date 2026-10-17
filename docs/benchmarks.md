# Ain backend benchmarks and AI repair loops

These Python 3 tools use the standard library, no third-party dependencies.

## Reproducible backend comparison

```sh
cargo build --bin ainc
python3 scripts/benchmark.py --strict --runs 5 --output results.json
```

The default examples are supported by **both** the Rust and LLVM backends.
The script measures compilation and median execution times, checks stdout and
exit-code equality, and emits JSON. Times are host-specific and noisy.

Source byte counts and whitespace-delimited units **are not LLM tokens**.
For real provider token counts, optionally supply JSONL model-trace records
containing `ok`, `prompt_tokens`, and `completion_tokens` with
`--model-trace path.jsonl`. Missing token counts are null, not guessed.

**Warning:** benchmarks execute compiled source. Use an OS-level sandbox for
untrusted code.

## AI generator compile/repair protocol

`scripts/agent_loop.py` calls a generator executable which reads JSON
from stdin and writes raw Ain source to stdout. On each failure, it passes
machine-readable `ainc check-json` diagnostics back to the generator.
The loop is bounded by `--attempts`, `--timeout`, and source byte
limit. It writes an output file only after the compiler accepts it.

```sh
python3 scripts/agent_loop.py \
  --prompt-file task.txt --output generated.ain \
  --ainc ./target/debug/ainc --attempts 4 \
  -- python3 my_model_adapter.py
```

A generator receives JSON like:
```json
{"protocol":"ain-generate-v1","prompt":"Print 42","attempt":1,"previous_source":"","diagnostics":[]}
```

For subsequent attempts, previous_source and diagnostics carry the last
failure. The generated code is **never executed** by this script.
Passing `check-json` proves neither functional correctness nor safety.
