# How the checks work, and what the Lean proofs cover

This page has the details behind the [README](https://github.com/san-digital/comparator-opt/blob/main/README.md). It covers the three checks that `comparator-opt verify` runs, what the Lean 4 proofs prove and what they leave out, and which files need human review for any of it to mean something.

## The three checks

### 1. The lock (`src/lock.rs`, `comparator.lock`)

These sit inside `@comparator:begin <id>` .. `@comparator:end <id>` blocks:

- in `src/kernels.rs`: each reference function and its `KernelSpec` impl (`is_valid`, which function `eval_ref` calls, `boundary_inputs`, `sample_input`, `bench_input`). `eval_fast` only calls the `fast_*` function outside the block;
- in `lean/ComparatorOpt/Challenge.lean`: the Lean definitions and theorem statements;
- in `lean/ComparatorOpt/Check.lean`: the binding of each proof to its statement (see below).

`comparator-opt verify` recomputes the SHA-256 digest of every block and fails if any block changed, is missing, or is new. `scripts/crosscheck.py` repeats this with a separate parser and Python's `hashlib`.

The scan is line-based. It proves the text between the markers is unchanged, not that the compiler builds that text, so the rest of each pinned file needs review too. See [Trusted base](#trusted-base).

### 2. The Lean axiom audit (`lean/replay.sh`, `lean/axioms.txt`)

`lean/replay.sh` compiles `Challenge.lean`, `Proofs.lean`, and `Check.lean` with stock Lean 4 (`Init` only, no Mathlib) at the version in `lean-toolchain`, replays them through `leanchecker -v`, and writes the `#print axioms` output for every audited theorem to `lean/axioms.txt`. It fails if any theorem depends on an axiom other than `propext`, `Classical.choice`, or `Quot.sound`. That rules out `sorryAx`, `Lean.ofReduceBool` (from `native_decide`), `Lean.trustCompiler`, and custom `axiom` declarations.

`Proofs.lean` is not pinned, so a proof there could quietly prove a weaker statement (for example `theorem ascii_lower_byte_sound : True := trivial`). The pinned `Check.lean` prevents this: it restates every target as a `ComparatorOpt.Audit` theorem whose proof is the matching `Proofs.lean` theorem. It only compiles if each proof has exactly the pinned type, and the `Audit` theorems are audited too, so anything Lean inserts to make a weaker proof fit shows up in their axiom list.

CI runs `replay.sh` and fails if the regenerated `lean/axioms.txt` differs from the committed one. `comparator-opt verify` then reads the committed file with `verify_lean_axiom_log`, which also handles axiom lists that Lean wraps across lines. Without CI, `verify` only reads the committed file and does not run Lean.

### 3. The differential tests (`src/harness.rs`)

`verify_kernel` checks that the fast path agrees with the reference on every boundary input, on 10,000 random inputs from a fixed SplitMix64 seed, and on the benchmark input. A proof that `fast x = ref x` for every valid `x` is no use if almost nothing counts as valid, so it also fails if `is_valid` accepts fewer than 50% of the random inputs (`min_valid_bps`).

## What the Lean proofs cover

All ten audited theorems (five proofs and their five `ComparatorOpt.Audit` bindings) depend only on `propext` and `Quot.sound`.

| Theorem (`Proofs.lean`) | Statement (`Challenge.lean`) | What it proves |
| :--- | :--- | :--- |
| `ascii_lower_byte_sound` | `AsciiLowerByteSound` | For all 256 `UInt8` values, the masking formula (`(b & 0x7f) + 0x3f` and `+ 0x25`, then the high bits) equals `if 65 <= b && b <= 90 then b + 32 else b`. |
| `swar_no_carry_add_sound` | `SwarNoCarryAddSound` | Packing eight numbers in base 256 is linear over `Nat`: `pack8 a + pack8 b = pack8 (a + b)`. This is close to trivial. It matches `u64` lane-wise addition only while every lane sum stays below `256` (at most `127 + 63 = 190` in the ASCII kernel), and that bound is not part of the theorem. |
| `utf8_lead_bit_sound` | `Utf8LeadBitSound` | For all 256 `UInt8` values, `(((~b) \| (b << 1)) & 0x80) >> 7` is `1` exactly when `(b & 0xc0) != 0x80`. |
| `horizontal_byte_sum_sound` | `HorizontalByteSumSound` | `((pack8(c0..c7) * 0x0101010101010101) / 2^56) % 256 = c0 + ... + c7` whenever the sum is below `256`. The Rust kernel keeps the sum at most `248` by working in blocks of 31 words (`31 * 8 = 248`). A version that sums 32 or more words before this step goes wrong once the total reaches `256`; a test checks that the harness catches this. |
| `mersenne31_fast_sound` | `Mersenne31FastSound` | Two folds (`x % 2^31 + x / 2^31`, twice, then map `2^31 - 1` to `0`) equal `x % (2^31 - 1)` for every `x < (2^31 - 1)^2`, about `4.6 * 10^18` values. |

## What they do not cover

- The proofs are about Lean definitions (`swarAsciiLowerByte`, `swarUtf8LeadBit`, `fastMersenne31Mod`, and the `ref*` definitions) that restate the Rust code by hand. Nothing checks that the Rust `fast_*` and `ref_*` functions match them. If someone edits a `fast_*` function, the proofs say nothing about the new code until a person updates the Lean copy, which is a pinned, reviewed change.
- The ASCII and UTF-8 theorems are about one byte. That the packed `u64` code applies them to each of the 8 lanes independently follows from the no-carry bounds in the comments in `src/kernels.rs`. Tests check every byte value in every lane position, but this step is not proved in Lean.
- The Mersenne proof works over unbounded `Nat`. That the Rust `u64` arithmetic does not overflow on the valid range is argued, not proved (the intermediate values stay below `2^32`).

## Trusted base

The checks only mean something while the code that runs them, and the specification they check against, are reviewed by a person. In this repository, a change by an optimizer should only touch the bodies of the `fast_*` functions in `src/kernels.rs` and `lean/ComparatorOpt/Proofs.lean`. Even those need a look:

- Rust code outside the pinned blocks can change what the compiler builds from them, for example by wrapping a block in a raw string literal or putting a `#[cfg]` attribute in front of it.
- The fast functions run in the same process as the checker, so they can do anything the checker can, including exiting early with a success status.
- `Proofs.lean` is compiled in CI with no sandbox, Lean can run arbitrary code at compile time (for example with `#eval`), and `Check.lean` imports it, so anything it adds to the Lean environment (such as a macro) is active when the pinned binding is checked.

Everything else needs review as a matter of course: `comparator.lock`, `src/lock.rs`, `src/harness.rs`, `src/bin/comparator_opt.rs`, `lean/replay.sh`, `lean-toolchain`, `lean/ComparatorOpt/Challenge.lean`, `lean/ComparatorOpt/Check.lean`, `scripts/crosscheck.py`, and `.github/workflows/ci.yml`. On GitHub, enforce this with `.github/CODEOWNERS` and branch protection that requires code-owner review.

The challenge and solution split follows Lean's [comparator](https://github.com/leanprover/comparator) tool, which its README says is now part of Lean (`lake comparator`, from v4.35.0). That tool builds the untrusted solution inside a sandbox and compares its exported theorems with the challenge. This repository's `replay.sh` compiles `Proofs.lean` directly and imports it into the pinned `Check.lean`, so it relies on review of `Proofs.lean`. Moving to `lake comparator` would address this, but needs a newer Lean than the one pinned here.

## Replaying the proofs locally

You need [elan](https://github.com/leanprover/elan); it installs the Lean version in `lean-toolchain`. No Mathlib checkout is needed.

```sh
./lean/replay.sh
git diff --exit-code -- lean/axioms.txt
cargo run --release -- verify .
```
