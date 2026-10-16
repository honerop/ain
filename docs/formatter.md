# Deterministic Ain source formatting

Ain is written exclusively by AI-generated source, so stable formatting
reduces noisy rewrites and makes code reviews, cache keys, and generated
test fixtures easier to compare.

Two CLI commands are supported:

```sh
ainc fmt main.ain                    # stdout, no in-place mutation
ainc fmt main.ain -o formatted.ain  # write formatted output
ainc fmt-check formatted.ain         # fails with nonzero exit code if unformatted
```

The parser validates source before formatting, so invalid Ain expressions
are rejected rather than silently reformatted. Every non-comment source
line is normalized to one space between lexical tokens and no leading or
trailing spaces. The formatter preserves the original order of functions
and statements, all comments (including inline comments), blank lines,
and the meaning of escaped string literals, and uses Unix LF line endings.

The formatter is **idempotent**: `fmt(fmt(source)) == fmt(source)`.
Whitespace between tokens does not affect Ain's fixed-arity prefix parsing.
It does not reorder functions or records, rewrite expressions, remove comments
or alter runtime behavior. Format checking is suitable for CI.

Tests cover byte-for-byte idempotence, comments containing string-like text,
strings containing comment markers and escape sequences, generated-program
result equivalence, CLI writing, and invalid-source rejection.
