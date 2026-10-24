# BABEL language guide (experimental)

BABEL is a compact, statically checked source language using prefix expressions. This guide describes the **currently implemented subset**, not a finalized language standard.

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

BABEL currently expects **one definition or expression per source line**. Prefix expressions are nested using function arity, not punctuation.

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

| BABEL value | Type |
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

Not all operations are available in the LLVM preview. For unsupported programs, use the Rust backend with `cargo run -- run file.babel`.

## Examples

- [Arithmetic](../examples/arithmetic.babel)
- [Recursion](../examples/recursive.babel)
- [Conditional expressions](../examples/branches.babel)
- [Strings](../examples/strings.babel)
- [Unit arguments](../examples/hello.babel)

## Current limitations

No collections, records, modules, lambdas, user generics, pattern matching, mutable variables, custom error types or general-purpose effect system yet. The grammar and syntax may change across minor releases.

## External process invocation (Rust and bootstrap C backends)

The generic `run` builtin executes one program with one argument, without
going through a shell. The value returned is the process exit code.

```text
status = run "echo" "Hello from Babel"
print status
```

For a more complex application, put native platform libraries and application
logic in a separate executable, not in the language compiler. The `run` builtin
can launch that executable without teaching Babel about GTK, browsers or AI providers.

**Security:** `run` can execute external programs with the user's permissions.
Do not run untrusted Babel code. This experimental builtin is not in the LLVM
backend yet.

## Native desktop GUI, AI, and browser (Rust native backend)

BABEL includes broad, reusable builtins so programs written by AI models can
create desktop apps without switching to Rust, Python or JavaScript.

```text
layout = "header\\tHello\\nlog\\tWelcome\\nentry\\tprompt\\tType here\\nbutton\\tsend\\tSend\\nvim\\t1\\n"
event = gui "Demo" layout
print event
```

`gui title layout` opens or updates a **native GTK4 window** and blocks until a
button, Enter key, or close event. Events are returned as `action\\ninput`, with
`quit\\n` for window close. The layout uses newline-separated records with
literal tab delimiters: `header<TAB>text`, `hint<TAB>text`,
`log<TAB>text`, `entry<TAB>id<TAB>placeholder`,
`button<TAB>id<TAB>caption`, and `vim<TAB>1` for Vim keys.
Babel code can recursively call `gui` to update content and handle events.

`chat provider model prompt` returns a text response through either `deepinfra`
(requires `DEEPINFRA_API_KEY` and curl/jq) or `chatgpt` (requires the official
Codex CLI and `codex login`). An empty model string picks the provider default.

`browser operation argument` maintains an isolated Chromium/Playwright session.
Operations: `open`, `read`, `links`, `back`, `click`, `fill`, `screenshot`.
The `fill` argument is formatted as `CSS_SELECTOR|value`. It requires Python
Playwright, Chromium and jq. Browser code and provider integration are built into
the language runtime rather than each application repository.

All three features are experimental, not yet supported by AIR/LLVM or the
self-hosted C backend, and are unsupported on platforms lacking their
dependencies. The GUI event loop currently blocks during network operations.

## Responsive GTK4 applications in Babel

The native backend supports `guiupdate TITLE LAYOUT` to render a GTK4 window
without blocking the Babel main thread and `guitick MILLISECONDS` to pump GTK
input events while asynchronous `spawn` tasks run. `guitick` accepts 0-250ms.
It returns `tick\\n` when no user event is ready or the normal `action\\ninput`
format used by `gui`. Calls must occur on the GTK main thread.

`guiupdate` rerenders the window and resets pending events; prefer `guitick`
while a task runs to preserve the user's partly typed input. Normal `gui`
remains the simple blocking convenience API. These operations do not magically
make synchronous `chat` calls nonblocking; spawn a typed Data callback that
invokes the provider, then poll its task with `ready` from a `guitick` loop.

### Additional GTK4 widgets

The native `gui` and nonblocking `guiupdate` layout strings support:

- `image\\tFILENAME\\tSIZE`: picture from a local file with a bounded square size
  (32..720 px). Requires the Babel `read` builtin capability when restricted.
- `progress\\tPERCENT\\tCAPTION`: progress bar with bounded 0..100 fraction
  and optional caption.
- `separator\\t`: horizontal separator between sections.
- `action\\tID\\tCAPTION`: vertical full-width button, unlike the bottom
  horizontal `button` row. Both emit the same `ID\\nINPUT` event format.

Images use GTK's native file loaders and must be explicitly available on the
machine. All content and event behavior stays in Babel code. These are incremental
widgets, not a general virtual DOM, style engine or comprehensive accessibility
framework.

## Terminal and CLI applications (Rust native backend)

Babel supports interactive and one-shot CLI applications without a GUI or extra scripting language:

- `argv _` returns a Data list of UTF-8 command-line arguments, excluding the executable path. Use `size`, `get` and `astext` to inspect it.
- `readline _` reads exactly one stdin line and returns a recoverable Data result with `ok`, `value`, `error` fields. Blank lines are valid; EOF returns `ok=false`, `error="end of input"`. Lines exceeding 1 MiB are rejected.
- `print` writes a line to stdout. Existing `input _` continues to read all stdin at once.

```babel
loop _ = receive readline _
receive value = if asbool field value "ok" handle astext field value "value" "done"
handle line = if == line ":quit" "done" loop print line
print encode argv _
print loop _
```

These generic primitives require no GTK4 dependency and are currently implemented by the Rust code-generation backend, not the experimental AIR/LLVM or bootstrap C backend.


## Portable GTK4 layout primitives

See [native layout containers, GTK CSS and platform setup](gui.md).
