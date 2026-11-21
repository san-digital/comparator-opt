# Zero-Trust Challenge Architecture and Lean 4 Proofs

`comparator-opt` separates human-maintained reference specifications from machine-synthesized or hand-tuned bit-twiddling fast paths.

## Three-Gate Comparator Protocol

1. **Cryptographic Challenge Lock (`src/lock.rs` + `comparator.lock`):**
   Reference functions (`ref_fn`), input validity guards (`ValidInput`), and Lean theorem targets (`lean/ComparatorOpt/Challenge.lean`) sit inside `@comparator:begin <id>` .. `@comparator:end <id>` blocks. `comparator-opt verify` recomputes FIPS 180-4 SHA-256 digests over every block and aborts on any byte change, missing pin, or added pin. Optimizers can rewrite fast paths and `Proofs.lean` freely while the specification remains immutable.
2. **Kernel Axiom Audit (`lean/replay.sh` + `lean/axioms.txt`):**
   `Proofs.lean` compiles against stock Lean 4 (`Init` only, zero Mathlib dependency) and replays through `leanchecker -v`. `verify_lean_axiom_log` inspects `#print axioms` output and permits only `[propext, Classical.choice, Quot.sound]`, rejecting `sorryAx`, `Lean.ofReduceBool` (`native_decide`), `Lean.trustCompiler`, and custom `axiom` declarations.
3. **Non-Vacuity and Differential Harness (`src/harness.rs`):**
   Even a sound proof of `forall x, ValidInput x -> fast x = ref x` is useless if `ValidInput` rejects production traffic. `verify_kernel` enforces a minimum acceptance ratio (`min_valid_bps`) over corpus traffic alongside deterministic boundary vectors and SplitMix64 differential sweeps.

## Verified Kernels (`lean/ComparatorOpt/`)

All five theorems use only `[propext, Quot.sound]`:

| Theorem (`Proofs.lean`) | Pinned Target (`Challenge.lean`) | What It Proves |
| :--- | :--- | :--- |
| `ascii_lower_byte_sound` | `AsciiLowerByteSound` | Exact equivalence across all 256 `UInt8` values between `(b & 0x7f) + 0x3f` / `+ 0x25` MSB masking and `if 65 <= b && b <= 90 then b + 32 else b` |
| `swar_no_carry_add_sound` | `SwarNoCarryAddSound` | Base-256 lane linearity across 8 packed bytes when per-lane sums stay `< 256` (`127 + 63 = 190 < 256`) |
| `utf8_lead_bit_sound` | `Utf8LeadBitSound` | Exact equivalence across all 256 `UInt8` values between `(((~b) \| (b << 1)) & 0x80) >> 7` and `(b & 0xc0) != 0x80` |
| `horizontal_byte_sum_sound` | `HorizontalByteSumSound` | Proves `((pack8(c0..c7) * 0x0101010101010101) / 2^56) % 256 = c0 + ... + c7` whenever `c0 + ... + c7 < 256` (enforced by 31-word blocks: `31 * 8 = 248 < 256`, preventing the classic 255-word top-byte overflow bug) |
| `mersenne31_fast_sound` | `Mersenne31FastSound` | Proves division-free two-fold reduction modulo `2^31 - 1` equals `x % (2^31 - 1)` for all `4.61 * 10^18` integers `x < (2^31 - 1)^2` |

## Replaying Locally

Requires only stock Lean 4 (no Mathlib checkout needed):

```sh
./lean/replay.sh
cargo run --release -- verify .
```
