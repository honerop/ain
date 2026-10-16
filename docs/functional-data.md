# Ain functional data foundation

Ain is a general-purpose language intended exclusively for AI-generated code.
It is **functional-first**, but deliberately permits explicitly controlled state
and I/O. An extensive builtin library is a design goal, not a reason to put
every feature directly into the parser.

## JSON and Data

The native Rust backend has a dynamically tagged Data type representing JSON
null, Boolean, number, string, list and record values.

* json STRING parses JSON into Data.
* tryjson STRING returns a record with ok (boolean), value (Data), and error (text).
* encode DATA produces canonical JSON with lexicographically sorted object keys.
* boxint, boxstr and boxbool convert Ain primitives to Data.
* asint, astext and asbool convert Data back to Ain primitives.
* array _, record _, nil _ create empty lists, records and null.
* push LIST ITEM and put RECORD KEY VALUE return updated values.
* get LIST INDEX, field RECORD KEY, hasfield RECORD KEY, keys RECORD,
  size COLLECTION, pair FIRST SECOND enable inspection and tuple-style data.

These lists/records are dynamically checked, not yet statically parameterized
types or user-defined structs. Invalid casts or accesses can fail at runtime.

## Higher-order functional transformations

Callback functions are named Ain functions whose names are passed as STRING
literals, to avoid the eager-call behavior of fixed-arity prefix parsing.

    double x = boxint * asint x 2
    even x = == % asint x 2 0
    add a b = boxint + asint a asint b
    values = push push push array _ boxint 1 boxint 2 boxint 3
    print encode map "double" values
    print encode filter "even" values
    print asint fold "add" boxint 0 values

map uses a Data -> Data callback; filter uses Data -> Bool;
fold uses Data -> Data -> Data. Callbacks are type-checked at compile time.
There are no first-class closures yet.

iterate "function" COUNT SEED invokes Data -> Data up to a million times.
each "function" LIST invokes a Data -> Unit side-effect callback.

## Optional mutable state

Use cell DATA for a reference cell, readstate CELL to read its Data value,
setstate CELL DATA to update it, and swapstate CELL DATA to exchange values.

    counter = cell boxint 0
    setstate counter boxint 5
    print asint readstate counter

Pure transformations are preferred; these cells cover explicit mutation where
needed. This is not yet an effect system or concurrency support.

## Machine diagnostics

Run: ainc check-json FILE.ain

On success: {"ok":true,"definitions":3}
On error: {"ok":false,"diagnostics":[{"severity":"error","line":2,"message":"..."}]}

Errors produce a nonzero exit status. Column spans and stable diagnostic
codes have not yet been implemented.

## Boundaries

The feature currently targets Ain's Rust code-generation backend, not its
experimental AIR/LLVM or self-hosted C backends. JSON parsing is strict
(up to 8 MiB input and 64 nesting levels); it rejects comments, trailing
commas, NaN and Infinity. Numbers preserve their textual representation.

End-to-end generated-binary regression tests are in tests/integration.rs.
