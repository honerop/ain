# OpenCode free provider

Babel's general-purpose `chat PROVIDER MODEL PROMPT` builtin recognizes
`opencode` for the installed OpenCode CLI.

```text
print chat "opencode" "" "Explain monads in Haskell"
```

An empty model defaults to `opencode/mimo-v2.6-flash-free`, the free model
tested with OpenCode's JSONL CLI mode. The provider is the OpenCode CLI,
**not** the separate OpenBot desktop application. OpenBot is an orchestration
workspace, not an inference API or model.

Babel launches `opencode run --model MODEL --format json --agent plan -- PROMPT`
as an argument-array subprocess (never through a shell), with an isolated
output capture and a 180-second timeout. It concatenates only text event
payloads from the JSONL stream, reports CLI errors, and limits output sizes.
The plan agent is the default for chat requests. This is **not** a guarantee
that the CLI, its plugins or global config will have no side effects, and no
untrusted task should be run without real OS confinement.

OpenCode must be installed and accessible as `opencode` on PATH; optional
`BABEL_OPENCODE_BIN` overrides the executable path for local testing.
Free models may change, have rate limits or require provider setup.

`chat "opencode"` requires Babel's `ai` builtin permission in restricted
mode, but permission gating is not an OS sandbox. Model authentication and
real provider availability cannot be tested in CI. Integration tests instead
use an executable mock OpenCode CLI to validate the exact arguments, JSONL
text extraction, defaults and meaningful failures.
