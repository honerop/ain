# Statically inferred tuple pairs in Ain

Ain supports heterogeneous tuple pairs without boxing values into dynamic
JSON `Data`. The compiler infers both element types, including nested lists,
maps, and tuples. These values are compiled to native Rust tuple types.

```text
person = tuple2 "Ada" 17
print tuplefst person
print tuplesnd person
print tupleswap person
```

- `tuple2 A B` constructs a statically typed pair `(A,B)`.
- `tuplefst PAIR` returns the first value with its exact type.
- `tuplesnd PAIR` returns the second value with its exact type.
- `tupleswap PAIR` returns `(B,A)`.

Ain's earlier `pair` builtin still constructs a *dynamic* two-item Data
list for JSON-style operations, so it is not interchangeable with a native
tuple.

The current tuple implementation handles **two-item tuples**, nested pairs,
and projections. There is no destructuring syntax for declarations and no
general user-defined struct/record syntax yet. AIR/LLVM and the bootstrapped C
subset reject these structured types; the native Rust backend supports them.

Generated-program tests verify mixed element types, native printing, nested
tuples, swapping, and compile-time rejection of invalid tuple operands.
