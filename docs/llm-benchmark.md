# Real LLM coding benchmark

The Ain benchmark runs **real model-generated programs** against input/output
tests instead of measuring source length alone. It can call an
**OpenAI-compatible Chat Completions API** or read existing response files. The
Python 3 runner uses only the standard library; no OpenCode or OpenAI SDK is
needed.

Supported: Ain, Python, Rust and TypeScript. The initial suite has six
tasks with multiple test cases each. Treat this as a starting corpus, not
evidence of a general language advantage.

## Quick start: any OpenAI-compatible API

From the repository root:

~~~sh
# Put your actual key in an environment variable, not command history.
export OPENAI_API_KEY="your-api-key"
cargo build --bin ainc

python3 tools/llm_bench.py \
  --api-responses-dir ./responses/my-model \
  --api-base-url https://api.openai.com/v1 \
  --model YOUR_MODEL_ID \
  --languages ain,python,rust,typescript \
  --attempts 3 \
  --ainc "$PWD/target/debug/ainc" \
  --report ./results/llm-benchmark.json \
  --allow-unsafe-execution
~~~

The base URL can be a vendor's compatible API prefix (e.g.
\`https://provider.example/api/v1\`) or an entire
\`/chat/completions\` endpoint. The benchmark appends
\`/chat/completions\` if needed. Only the Chat Completions JSON protocol is
implemented, not the OpenAI Responses API.

**IMPORTANT:** Generated programs are not sandboxed. Compilation and execution
can run arbitrary commands with your OS permissions. Only run that evaluation
command in a properly isolated VM/container with filesystem and network limits.
The opt-in flag only acknowledges the risk; it does not provide isolation.

If you want **just API generation, without executing generated code**, omit the
execution acknowledgement and add \`--generate-only\`:

~~~sh
python3 tools/llm_bench.py \
  --api-responses-dir ./responses/my-model \
  --api-base-url https://api.openai.com/v1 \
  --model YOUR_MODEL_ID \
  --attempts 3 \
  --generate-only
~~~

Then evaluate the saved responses later in an isolated environment:

~~~sh
python3 tools/llm_bench.py \
  --responses-dir ./responses/my-model \
  --report ./results/llm-benchmark.json \
  --attempts 3 \
  --allow-unsafe-execution
~~~

### Other compatible providers / local models

Use a different key env var without exposing the key as a CLI argument:

~~~sh
export MY_PROVIDER_TOKEN="..."
python3 tools/llm_bench.py \
  --api-responses-dir ./responses/vendor-model \
  --api-base-url https://your-provider.example/v1 \
  --api-key-env MY_PROVIDER_TOKEN \
  --model vendor/model-id \
  --generate-only
~~~

For a local, unauthenticated Chat Completions server:

~~~sh
python3 tools/llm_bench.py \
  --api-responses-dir ./responses/local \
  --api-base-url http://localhost:1234/v1 \
  --model local-model-name \
  --no-api-key \
  --generate-only
~~~

Plain HTTP is accepted automatically for loopback addresses. For nonlocal
HTTP, \`--allow-insecure-http\` is required and not recommended.

### Optional API controls

- \`--temperature 0.2\` sends a sampling temperature; by default it is
  omitted because some models reject explicit temperatures.
- \`--max-output-tokens 2048\` sends \`max_completion_tokens\` (including
  hidden reasoning tokens for models that use them). Providers requiring
  \`max_tokens\` can use \`--max-tokens-field max_tokens\`. Default: no limit
  field.
- \`--api-timeout 120\` sets the request timeout in seconds.
- \`--api-retries 2\` retries explicit HTTP 429/5xx responses. Default: 0,
  because retries can result in additional charges. Ambiguous connection
  failures are never retried automatically.
- \`--languages ain,python\` narrows the comparison; \`--attempts 1\` controls
  independent generation attempts per task. Existing response files are
  reused when the requested model is unchanged. Use a *different directory*
  when you change models or providers.

No key is written to response JSON or reports. API usage values are saved
**only if the provider returned both prompt and completion token counts**.
If the provider does not report them, usage remains \`null\`; no estimation is
made. Individual response files also preserve raw model output, chosen model
and finish reason. Different providers may count tokens differently.

## Manual response workflow

Export prompts with no API or program execution:

~~~sh
python3 tools/llm_bench.py --export-prompts prompts
~~~

Prompts are stored under \`prompts/<task-id>/<language>/prompt.txt\`. Test cases
are not included. Write responses under
\`responses/<task-id>/<language>/1.json\`:

~~~json
{
  "source": "print(1)\n",
  "provider": null,
  "model": null,
  "usage": null,
  "generation_seconds": null
}
~~~

A generation result may also contain actual usage such as:

~~~json
"usage": {"input_tokens": 120, "output_tokens": 35}
~~~

Do not fabricate token values. For publishable comparisons, fix the prompt
and model settings, run multiple attempts, and keep model transcripts.

## What is measured

- A candidate passes only if it compiles when required, exits successfully,
  and exactly matches stdout for every test case.
- First-attempt success, any success within the attempt budget, compile
  failures, individual test outcomes and process wall-time are saved.
- The attempt-budget metric is **not** statistical \`pass@k\`.
- Generation time and provider-reported token usage are recorded; they are
  not estimated from source text. Execution time includes process startup.
- Public benchmark task fixtures are not secret if the model already has
  access to the repository. For rigorous studies, use separately maintained
  private task sets not exposed to the model.

Source outputs are limited to 100 KiB, API JSON bodies to 2 MiB,
candidate stdout/stderr are truncated in the report, and all subprocesses
have time limits. The tool is still **not a security sandbox**.

## Local tests (no external API access)

~~~sh
python3 -m unittest discover -s tests -p 'test_llm_*.py' -v
~~~

CI runs those tests using mocked API requests, then exercises the Ain
compiler on trusted test source. That checks the harness, not a paid
model's ability to generate Ain. Separately,
[\`token-benchmark.md\`](token-benchmark.md) compares source tokens only.
