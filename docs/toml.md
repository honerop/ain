# TOML parsing and serialization in Babel

Babel's native Rust backend exposes `tomlparse TEXT` and `tomlwrite DATA`.
Both return recoverable `Data` results (`ok`, `value`, `error`) instead
of terminating on invalid content.

```text
config = tomlparse "name = \"Babel\"\nport = 8080\n[features]\nunsafe = false"
print asbool field config "ok"
print asint field field config "value" "port"
out = tomlwrite field config "value"
print astext field out "value"
```

Supported TOML constructs include ordinary single-quoted and double-quoted
strings, booleans, integers/decimals/exponents, nested arrays, bare and
dotted keys, TOML comments, and tables introduced by `[section]` headers.
The writer outputs keys and nested tables in sorted order, so generated
configuration is deterministic.

**This is an intentionally strict TOML subset**, not a full TOML 1.0 parser.
Unsupported syntax includes date/time literals, inline tables, arrays of
tables, multiline strings, special floats, and quoted keys. The parser
rejects duplicate keys, duplicate tables, unknown values and invalid nesting
with line-number diagnostics. Maximum input and output sizes are 4 MiB; array
nesting is limited to 64.

This is suitable for many simple application configuration files, not for
arbitrary existing TOML documents requiring all language features. The
standard library loads only for programs using `tomlparse`/`tomlwrite`.
End-to-end tests include nested tables, quoted text, deterministic output,
roundtrips, malformed values and compile-time type errors.
