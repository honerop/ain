# Statically typed lists in Ain

Ain is functional-first, not purely functional, and has both dynamic JSON-style
Data lists and statically inferred, homogeneous native lists.

```text
double x = * x 2
even x = == % x 2 0
add x y = + x y
numbers = listpush listpush listof 1 2 3
print listmap "double" numbers
print listfilter "even" numbers
print listfold "add" 0 numbers
print listat numbers 1
print listlen numbers
```

Available functions:

- `listof VALUE` creates a native list containing one value.
- `listpush LIST VALUE` returns a new list. Type checks element consistency.
- `listat LIST INDEX` returns an element with compile-time inferred type.
- `listlen LIST` returns the number of elements.
- `listmap "callback" LIST` applies a named function, returning a typed list.
- `listfilter "predicate" LIST` returns a typed filtered list.
- `listfold "callback" INITIAL LIST` accumulates a value of the initial type.

Because the expression language uses prefix notation with eagerly evaluated
function arguments, callbacks are provided as quoted function names. They are
resolved and checked at compile time. First-class function values, closures,
and capturing environment variables are **not** supported yet.

The internal type system now includes recursive List<T> and an occurs check,
and rejects mixed element types before generating code. Lists can be nested.
This is a **typed list foundation**, not the complete generics/typeclass/ADT
system tracked in issue #8.

Lists are currently compiled to Rust vectors. The AIR/LLVM and bootstrap C
backends do not yet support them. Memory/runtime behavior is native Rust code,
and no new crates or GTK/browser dependencies are needed.

Tests run compiled binaries with integer, string and nested lists, map/filter/
fold, element type errors and callback signature checks.
