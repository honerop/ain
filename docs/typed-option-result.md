# Typed Option and Result in Ain

Ain uses functional-first expressions and supports recoverable error values
alongside explicitly effectful operations.

## Optional values

```text
known = option_some 10
missing = option_none _
print option_get known 2
print option_get missing 9
print option_has known
```

- `option_some VALUE` constructs an Option<T>.
- `option_none _` constructs a missing optional value. Its element type
  must be inferable from context.
- `option_has OPTION` tests presence.
- `option_get OPTION FALLBACK` returns the typed element or the fallback.

## Recoverable results

```text
success = result_ok 42
failure = result_err "failed"
print result_valid success
print result_get failure 0
print result_error failure
```

- `result_ok VALUE`: Result<T, String> with successful value.
- `result_err MESSAGE`: Result<T, String> error; value type inferred from use.
- `result_valid RESULT`: Boolean.
- `result_get RESULT FALLBACK`: T or the fallback T.
- `result_error RESULT`: error text or empty string.

`tryread PATH` is the first native I/O function returning a typed
Result<String,String> rather than panicking. If
`AIN_RESTRICTED=1` blocks file reads it returns an error value instead of
reading the path.

These are not yet generalized algebraic data types with user declarations,
pattern matching, higher-kinded types, or effects in the type system.
Ain currently infers types from use and rejects incompatible fallbacks at
compile time. Some unconstrained `option_none` and `result_err` expressions
still need a type context.

The native Rust codegen handles Option and Result; AIR/LLVM and the
self-hosted C subset do not support these types yet.

End-to-end tests cover successful and missing options, result failures,
missing file reads, fallbacks, and compile-time type errors.
