#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LEAN_SRC="$REPO_ROOT/lean"

# Run from the repository root so elan picks the toolchain pinned in `lean-toolchain`.
cd "$REPO_ROOT"

if [[ -n "${LEAN_BIN_DIR:-}" ]]; then
  export PATH="$LEAN_BIN_DIR:$PATH"
fi

BUILD_DIR="$(mktemp -d "${TMPDIR:-/tmp}/comparator-opt-lean.XXXXXX")"
trap 'rm -rf "$BUILD_DIR"' EXIT
mkdir -p "$BUILD_DIR/ComparatorOpt"

export LEAN_PATH="$BUILD_DIR:${LEAN_PATH:-}"

for mod in Challenge Proofs Check; do
  echo "[lean] compiling ComparatorOpt.${mod}"
  lean -R "$LEAN_SRC" -o "$BUILD_DIR/ComparatorOpt/${mod}.olean" "$LEAN_SRC/ComparatorOpt/${mod}.lean"
done

echo "[leanchecker] replaying ComparatorOpt modules"
leanchecker -v ComparatorOpt.Challenge ComparatorOpt.Proofs ComparatorOpt.Check

echo "[axioms] writing verified axiom log to lean/axioms.txt"
AXIOM_OUT="$(lean --stdin <<'EOS'
import ComparatorOpt.Check
#print axioms ComparatorOpt.ascii_lower_byte_sound
#print axioms ComparatorOpt.swar_no_carry_add_sound
#print axioms ComparatorOpt.utf8_lead_bit_sound
#print axioms ComparatorOpt.horizontal_byte_sum_sound
#print axioms ComparatorOpt.mersenne31_fast_sound
#print axioms ComparatorOpt.Audit.ascii_lower_byte_sound
#print axioms ComparatorOpt.Audit.swar_no_carry_add_sound
#print axioms ComparatorOpt.Audit.utf8_lead_bit_sound
#print axioms ComparatorOpt.Audit.horizontal_byte_sum_sound
#print axioms ComparatorOpt.Audit.mersenne31_fast_sound
EOS
)"
echo "$AXIOM_OUT"
printf "%s\n" "$AXIOM_OUT" > "$LEAN_SRC/axioms.txt"

# Long axiom lists wrap across lines, so join the output before extracting each list.
# Anything outside the three standard axioms (sorryAx, Lean.ofReduceBool from
# native_decide, Lean.trustCompiler, or a custom axiom) fails the audit.
NONSTANDARD="$(printf "%s\n" "$AXIOM_OUT" | tr "\n" " " | grep -o "\[[^]]*\]" | tr -d "[]" \
  | tr "," "\n" | sed "s/^ *//; s/ *$//" \
  | grep -v -x -e "" -e "propext" -e "Classical.choice" -e "Quot.sound" || true)"
if [[ -n "$NONSTANDARD" ]]; then
  echo "error: non-standard axioms in footprint: $(echo $NONSTANDARD)" >&2
  exit 1
fi

echo "[ok] all ComparatorOpt Lean 4 proofs compiled, kernel-replayed, and axiom-audited"
