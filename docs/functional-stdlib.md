# Functional standard-library operations

Ain remains functional-first, but not purely functional. AI-generated code can
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
