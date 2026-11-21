# Zero-Trust Challenge Architecture and Lean 4 Proofs

`comparator-opt` separates human-maintained reference specifications from machine-synthesized or hand-tuned bit-twiddling fast paths.

## Three-Gate Comparator Protocol

1. **Cryptographic Challenge Lock (`src/lock.rs` + `comparator.lock`):**
   Reference functions (`ref_fn`), each kernel's `KernelSpec` wiring (`is_valid`, which reference `eval_ref` calls, `boundary_inputs`, `sample_input`, `bench_input`), Lean theorem targets (`lean/ComparatorOpt/Challenge.lean`), and the statement binding (`lean/ComparatorOpt/Check.lean`) sit inside `@comparator:begin <id>` .. `@comparator:end <id>` blocks. `comparator-opt verify` recomputes FIPS 180-4 SHA-256 digests over every block and aborts on any byte change, missing pin, or added pin. Optimizers can rewrite the `fast_*` functions and `Proofs.lean` freely while the specification remains immutable. In the `KernelSpec` impls, `eval_fast` only delegates to the unpinned fast path.
2. **Kernel Axiom Audit (`lean/replay.sh` + `lean/axioms.txt`):**
   `Proofs.lean` compiles against stock Lean 4 (`Init` only, zero Mathlib dependency, version pinned in `lean-toolchain`) and replays through `leanchecker -v`. Because `Proofs.lean` is editable, the pinned `Check.lean` restates every target as a `ComparatorOpt.Audit` theorem proved by the matching `Proofs.lean` theorem: it compiles only if each proof has exactly its pinned type, and the audit covers those theorems, so anything the elaborator inserts to make a weaker theorem fit (such as a coercion built on `sorry`) shows up in their footprint. `replay.sh` rewrites `lean/axioms.txt`; CI then fails if it differs from the committed copy and runs `comparator-opt verify`, whose `verify_lean_axiom_log` reads the `#print axioms` output (including lists that Lean wraps across lines) and permits only `[propext, Classical.choice, Quot.sound]`, rejecting `sorryAx`, `Lean.ofReduceBool` (`native_decide`), `Lean.trustCompiler`, and custom `axiom` declarations.
3. **Non-Vacuity and Differential Harness (`src/harness.rs`):**
   Even a sound proof of `forall x, ValidInput x -> fast x = ref x` is useless if `ValidInput` rejects production traffic. `verify_kernel` enforces a minimum acceptance ratio (`min_valid_bps`) over corpus traffic alongside deterministic boundary vectors and SplitMix64 differential sweeps.

## Trusted Base

The gates hold only while the code that runs them is trusted. An optimizer's change should touch nothing but the `fast_*` functions in `src/kernels.rs` (outside the pinned blocks) and `lean/ComparatorOpt/Proofs.lean`. Everything else, including `src/lock.rs`, `src/harness.rs`, `src/bin/comparator_opt.rs`, `lean/replay.sh`, `lean-toolchain`, and `.github/workflows/ci.yml`, needs human review, for example through a `CODEOWNERS` rule.

## Verified Kernels (`lean/ComparatorOpt/`)

All ten audited theorems (the five proofs and their five pinned `ComparatorOpt.Audit` bindings) use only `[propext, Quot.sound]`:

| Theorem (`Proofs.lean`) | Pinned Target (`Challenge.lean`) | What It Proves |
| :--- | :--- | :--- |
| `ascii_lower_byte_sound` | `AsciiLowerByteSound` | Exact equivalence across all 256 `UInt8` values between `(b & 0x7f) + 0x3f` / `+ 0x25` MSB masking and `if 65 <= b && b <= 90 then b + 32 else b` |
| `swar_no_carry_add_sound` | `SwarNoCarryAddSound` | Base-256 packing is linear over `Nat`: `pack8 a + pack8 b = pack8 (a + b)`. This matches `u64` lane-wise addition only while every lane sum stays below `256` (`127 + 63 = 190` in the ASCII kernel), which the kernel guarantees but Lean does not model |
| `utf8_lead_bit_sound` | `Utf8LeadBitSound` | Exact equivalence across all 256 `UInt8` values between `(((~b) \| (b << 1)) & 0x80) >> 7` and `(b & 0xc0) != 0x80` |
| `horizontal_byte_sum_sound` | `HorizontalByteSumSound` | Proves `((pack8(c0..c7) * 0x0101010101010101) / 2^56) % 256 = c0 + ... + c7` whenever `c0 + ... + c7 < 256` (enforced by 31-word blocks: `31 * 8 = 248 < 256`, preventing the classic 255-word top-byte overflow bug) |
| `mersenne31_fast_sound` | `Mersenne31FastSound` | Proves division-free two-fold reduction modulo `2^31 - 1` equals `x % (2^31 - 1)` for all `4.61 * 10^18` integers `x < (2^31 - 1)^2` |

The ASCII and UTF-8 theorems are per-byte statements. That the packed `u64` kernels apply them lane by lane follows from the carry-free bounds above; the tests check every byte value in every lane position, but this step is not formalized.

## Replaying Locally

Requires only [elan](https://github.com/leanprover/elan); the Lean version comes from `lean-toolchain` (no Mathlib checkout needed):

```sh
./lean/replay.sh
git diff --exit-code -- lean/axioms.txt
cargo run --release -- verify .
```
