# Assertions and native Ain tests

Ain supports a compact, statically type-checked assertion builtin:

```text
assert == + 20 22 42 "arithmetic must be correct"
assert not false "boolean logic"
print "tests passed"
```

`assert CONDITION MESSAGE` requires a Boolean condition and a text message,
and has result type Unit. Successful assertions continue program execution;
a failed assertion terminates the generated native program with a nonzero
exit code and emits the supplied message on stderr.

Ain-generated assertions are designed for smoke tests, generated test
fixtures, executable invariants and small compile/run/repair loops. They do
not yet provide a full dedicated test discovery runner, property-based tests,
shrinking, mocking, or coverage reporting.

End-to-end tests compile real Ain source and verify success, type mismatches,
and a native process failure on a false assertion. This builtin currently
targets Ain's Rust code-generation backend. The C bootstrap and experimental
AIR/LLVM backends do not implement it.
