# AIN language guide (experimental)

AIN is a compact, statically checked source language using prefix expressions. This guide describes the **currently implemented subset**, not a finalized language standard.

## Definitions and functions

```text
x = 10
double n = * n 2
add a b = + a b

print double x
print add 2 3
```

- `x = 10` binds a top-level value.
- `double n = ...` declares a single-argument function.
- `add a b = ...` declares a two-argument function.
- Calls contain no parentheses: `add 2 3`.
- All functions have fixed arity, discovered before expressions are parsed.
- `_` denotes a unit value or unit parameter. A unit-argument function such as `say _ = print "hi"` is called as `say _`.

AIN currently expects **one definition or expression per source line**. Prefix expressions are nested using function arity, not punctuation.

## Prefix expressions

```text
square x = * x x
hypot2 a b = + * a a * b b
print hypot2 3 4
```

The last line prints `25`.

`if` is a fixed-arity expression, not a statement:

```text
absdiff a b = if > a b - a b - b a
print absdiff 10 3
```

The compiler's `if`, `and` and `or` forms are short-circuiting, so the unused branch isn't evaluated.

## Types

The Rust compiler infers **monomorphic primitive types** from literals, operations and function usage:

| AIN value | Type |
| --- | --- |
| `42` | signed 64-bit integer |
| `true` or `false` | Boolean |
| `"hello"` | String |
| `_` | Unit |

Type inference is not yet Hindley–Milner polymorphism or a general generic-type system. Some underconstrained definitions are rejected.

Integer arithmetic is checked for overflow in the available backend paths. Division by zero is an error; no implicit coercions between unrelated primitive types are guaranteed.

## Common operations

| Group | Examples |
| --- | --- |
| Arithmetic | `+ - * / % neg abs pow min max clamp` |
| Logic | `== != < <= > >= and or not if` |
| String (Rust backend) | `concat len upper lower trim contains starts ends replace` |
| Parsing (Rust backend) | `num isnum word skip before after str` |
| I/O (Rust backend) | `print`, `input _` |

Each operator has a defined arity. For example, `+ 1 2` is valid but `+ 1` is incomplete.

Not all operations are available in the LLVM preview. For unsupported programs, use the Rust backend with `cargo run -- run file.ain`.

## Examples

- [Arithmetic](../examples/arithmetic.ain)
- [Recursion](../examples/recursive.ain)
- [Conditional expressions](../examples/branches.ain)
- [Strings](../examples/strings.ain)
- [Unit arguments](../examples/hello.ain)

## Current limitations

No collections, records, modules, lambdas, user generics, pattern matching, mutable variables, custom error types or general-purpose effect system yet. The grammar and syntax may change across minor releases.

## Desktop application host (Rust backend only)

The `desktop` builtin accepts three strings: a window title, a system prompt,
and the path to a local HTML document. It starts a loopback-only HTTP server,
prints its address, and opens the UI in the default browser via `xdg-open`:

```text
desktop "Ainex" "You are a general-purpose assistant." "web/index.html"
```

The document can call `POST /chat` with form-encoded `provider`, `model`,
and `prompt` values, and the `X-Ainex-Client: 1` header. Built-in providers
are `deepinfra` (requires `DEEPINFRA_API_KEY` and curl/jq) and `chatgpt`
(requires an installed, authenticated Codex CLI). No credentials are stored
in the generated executable or checked into source control.

This is an experimental minimal desktop host, not a general GUI widget library.
It currently runs only with `ainc run` / `ainc build`, not `run-llvm`.
The bootstrap C backend registers the builtin but fails explicitly if invoked.
