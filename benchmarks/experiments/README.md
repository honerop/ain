## Token-minimal Babel list syntax experiment

This directory holds three **real, compiling** candidate sources for the same
list length/mapping task as `benchmarks/tokens/lists/babel.babel`.
They test aliases, literals, and avoiding an unnecessary top-level binding,
respectively. `babelc tokens` measures real `o200k_base` source tokens for
all versions. Compare with the baseline before claiming improvement.
