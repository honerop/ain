# Functional standard-library operations

Babel remains functional-first, but not purely functional. AI-generated code can
use built-in operations for common transformations without loops or imports.

## Strings and text

- `split TEXT DELIMITER` gives a typed List<String>. An empty delimiter
  splits into Unicode scalar characters.
- `lines TEXT` gives lines as a typed List<String>.
- `strjoin LIST_OF_STRINGS DELIMITER` returns a String.

## Homogeneous typed lists

- `listreverse LIST`: reverse list elements.
- `listtake LIST N` / `listdrop LIST N`: take or drop a prefix. Negative
  counts are treated as zero.
- `listappend FIRST SECOND`: concatenate two lists with the same element type.
- `listflatten NESTED_LIST`: turn List<List<T>> into List<T>.
- `sortint LIST` and `sortstr LIST`: sort integer or string lists.

```text
items = split "a,b,c" ","
print strjoin items " | "
numbers = listpush listpush listof 3 1 2
print sortint numbers
print listtake numbers 2
```

Operations infer and check types at compilation. They operate on the native
Rust-backend `List<T>` collections, not JSON Data lists. Their implementation
is emitted inline with standard Rust iterator and collection functions and
does not add third-party library dependencies.

This is a focused standard-library expansion, not a general polymorphic
iterator/typeclass/closure system. Unicode grapheme segmentation, locale-aware
sorting, streaming text operations, and arbitrary comparator callbacks are
future work.

Generated-code tests verify transforms on integer/string/nested lists,
Unicode text and compile-time rejection of invalid combinations.

## Safe list access and typed combinations

`listhead xs` and `listlast xs` return `Option<T>` instead of panicking
on empty lists. Use `option_get xs fallback` to recover a value of the same
element type.

`listzip xs ys` returns a `List<Tuple<A,B>>`, truncated to the shorter
input. `listenumerate xs` returns a `List<Tuple<Int,T>>` with zero-based
indices. Element types are inferred and checked. These operations are
currently available in the Rust backend only.

```text
xs = listpush listof 10 20
ys = listpush listof "a" "b"
print tuplesnd listat listzip xs ys 1
print option_get listhead xs 0
print tuplefst listat listenumerate xs 1
```
