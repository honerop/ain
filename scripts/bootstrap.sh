#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p build
if [[ "${1:-}" == "--reference" ]]; then
  echo '[stage0] Independent Python reference -> C (offline validation)'
  python3 - <<'PYVERIFY'
from pathlib import Path
from scripts.stage0_reference import verify_primitive_types
signatures = verify_primitive_types(Path('bootstrap/compiler.ain').read_text())
assert len(signatures) >= 40
print(f'[type-audit] {len(signatures)} self-hosted functions resolve to primitive types')
PYVERIFY
  python3 scripts/stage0_reference.py bootstrap/compiler.ain -o build/stage0.c
  gcc -std=gnu11 -O1 -I runtime -w build/stage0.c -o build/stage0
else
  echo '[stage0] Rust AIN compiler -> native AIN bootstrap compiler'
  cargo run --quiet -- build bootstrap/compiler.ain -o build/stage0
fi
printf '[stage1] AIN bootstrap compiler -> C\n'
./build/stage0 < bootstrap/compiler.ain > build/stage1.c
gcc -std=gnu11 -O1 -I runtime -w build/stage1.c -o build/stage1
printf '[stage2] Stage1 compiler recompiles its own AIN source\n'
./build/stage1 < bootstrap/compiler.ain > build/stage2.c
gcc -std=gnu11 -O1 -I runtime -w build/stage2.c -o build/stage2
printf '[stage3] Stage2 compiler recompiles its own AIN source\n'
./build/stage2 < bootstrap/compiler.ain > build/stage3.c
cmp build/stage1.c build/stage2.c
cmp build/stage2.c build/stage3.c
printf '[verified] byte-identical generated C across stages 1, 2 and 3\n'
for file in examples/*.ain; do
  # The desktop example requires GTK4 and an interactive session; the
  # bootstrap proof only executes the portable primitive subset.
  if [[ "$file" == "examples/desktop.ain" || "$file" == "examples/gui.ain" || "$file" == "examples/gui_tick.ain" || "$file" == "examples/gui_rich.ain" ]]; then
    printf '[skip] %-35s GTK4 desktop (Rust backend only)\n' "$file"
    continue
  fi
  ./build/stage2 < "$file" > build/example.c
  gcc -std=gnu11 -O1 -I runtime -w build/example.c -o build/example
  ./build/example > build/example.out
  case "$file" in
    examples/arithmetic.ain) printf '25\n' > build/expected.out ;;
    examples/branches.ain) printf '46\n6\n' > build/expected.out ;;
    examples/hello.ain) printf 'I\n' > build/expected.out ;;
    examples/strings.ain) printf 'Hello, AGENT\n6\na_b_c\n' > build/expected.out ;;
    examples/recursive.ain) printf '720\n42\n' > build/expected.out ;;
    *) printf 'unknown example: %s\n' "$file" >&2; exit 1 ;;
  esac
  cmp build/example.out build/expected.out
  printf '[test] %-35s PASS\n' "$file"
done
