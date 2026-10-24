# Inferred parametric functions

Babel supports **inferred rank-1 parametric function polymorphism** through the
Rust code-generation backend. No `fn` keyword, annotations, or type parameter
punctuation is required.

```text
id x = x
print id 42
print id "hello"
print id true
```

Calls to a generalized function get independent type instantiations. Generic
signatures can contain `List<T>`, `Option<T>`, tuples, maps, and other
combinations inferred from the function body. Forward-referenced functions
work as well. Recursive and mutually recursive definitions are checked as
monomorphic recursive groups, then generalized at their external boundaries;
polymorphic recursion is not supported.

The Rust backend emits generic Rust functions rather than duplicating source.
Trait constraints for repeated values, direct equality and direct text output
are inferred conservatively; some higher-order trait interactions remain
unsupported. This feature does not imply user-defined structs, ADTs, or higher
ranked polymorphism. LLVM deliberately reports an unsupported feature diagnostic
when a polymorphic function is found rather than emitting invalid IR. The C
bootstrap uses tagged runtime values and handles the primitive subset.
