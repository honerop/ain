# Typed maps in Ain

Ain's native backend supports **statically inferred homogeneous maps with String keys**.
Unlike dynamic JSON objects, the compiler ensures every value in a map has a compatible type.

```text
scores = mapset mapone "Ada" 17 "Keth" 22
print mapat scores "Ada" 0
print maphas scores "Nobody"
print mapkeys scores
print mapvalues scores
print mapremove scores "Keth"
```

- `mapone KEY VALUE` creates one entry.
- `mapset MAP KEY VALUE` returns a map with one key set or replaced.
- `mapat MAP KEY FALLBACK` returns the value or a statically checked fallback.
- `maphas MAP KEY`, `maplen MAP`, `mapkeys MAP`, `mapvalues MAP`, `mapremove MAP KEY` provide lookup and persistent-style updates.

Keys are always UTF-8 strings and ordered lexicographically. Values can include
integers, booleans, strings, nested maps and typed lists; values must have the
same inferred type. A call does not mutate its input. The map is implemented with
Rust's `BTreeMap<String, T>`, linked only when map operations are used.

This is not yet a general-purpose `Map<K,V>` over arbitrary key types, a
user-defined record/struct system, or a closure/generic function mechanism.
Backend support currently covers native Rust output; AIR/LLVM and the C bootstrap
still exclude typed collection operations. See the integration tests for
positive/negative type checking and deterministic ordering.
