# comparator-opt

[![CI](https://github.com/san-digital/comparator-opt/actions/workflows/ci.yml/badge.svg)](https://github.com/san-digital/comparator-opt/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/comparator-opt.svg)](https://crates.io/crates/comparator-opt)
[![docs.rs](https://img.shields.io/docsrs/comparator-opt)](https://docs.rs/comparator-opt)

comparator-opt lets you accept a fast, hard-to-read version of a function (hand-tuned, or written by an optimizer or a coding agent) only if it matches a short, readable reference version. It is for Rust developers who want bit-twiddling speedups without having to check bit-twiddling code by eye.

This is a reference implementation of the pattern, with three example kernels. The `comparator-opt` command only checks this repository. To use the pattern on your own code, use the library: see [Use it on your own code](#use-it-on-your-own-code).

No dependencies, no `unsafe`, Rust 1.81 or later.

## Try it

```sh
git clone https://github.com/san-digital/comparator-opt
cd comparator-opt
cargo run --release -- verify .
```

Example output (Apple Silicon, rustc 1.99):

```text
[1/3] challenge lock verified (7 pinned blocks match SHA-256)
[2/3] lean/axioms.txt lists 10 theorems using only standard axioms (lean/replay.sh regenerates it)
[3/3] running boundary, non-vacuity, and differential tests:
  kernel=ascii_lower_swar       valid=10000/10000 (10000 bps) ref= 16296.6 ns  fast=   888.9 ns  speedup=18.33x
  kernel=utf8_char_count_swar   valid=10000/10000 (10000 bps) ref= 16590.9 ns  fast=   443.4 ns  speedup=37.42x
  kernel=mersenne31_mod_fold    valid=8757/10000 (8757 bps) ref=  5820.7 ns  fast=  1195.5 ns  speedup= 4.87x
verify: ok
```

It exits with `0` if every check passes, `1` if one fails, and `2` for a usage error. `valid` is how many random inputs passed the kernel's validity check; the Mersenne kernel deliberately generates some out-of-range batches. Timings vary a lot between machines and between runs, and the speedups need context: see [Limits](#limits).

The three example kernels:

| Kernel | Readable reference | Fast version |
| :--- | :--- | :--- |
| `ascii_lower_swar` | lowercase each byte with an `if` | 8 bytes at a time in a `u64` |
| `utf8_char_count_swar` | count bytes where `(b & 0xc0) != 0x80` | 8 bytes at a time, summed in 248-byte blocks so the byte counters cannot overflow |
| `mersenne31_mod_fold` | `x % (2^31 - 1)` | two shift-and-add folds, for `x < (2^31 - 1)^2` |

`cargo install comparator-opt` also works, but the installed binary has these kernels compiled in and fails anywhere except a checkout of this repository. Inside a checkout, prefer `cargo run`: an installed binary tests its own copy of the kernels, not the code on disk.

## How it works

Each kernel has a readable reference function (`ref_*`) and a fast function (`fast_*`) in [`src/kernels.rs`](https://github.com/san-digital/comparator-opt/blob/main/src/kernels.rs). The reference, together with the test setup (which inputs are valid, edge cases, a random input generator, and a benchmark input), sits between `// @comparator:begin <id>` and `// @comparator:end <id>` markers. The fast function sits outside the markers. The fast function is the part meant to change; the rest is the specification.

`verify` runs three checks:

1. [`comparator.lock`](https://github.com/san-digital/comparator-opt/blob/main/comparator.lock) holds the SHA-256 digest of every marked block. `verify` fails if a block was edited, removed, or added. This stops a change that "fixes" a failing test by editing the reference or the test inputs from slipping through unnoticed.
2. For the bit tricks, there are Lean 4 proofs that the formulas are right (stock Lean, no Mathlib). `verify` reads [`lean/axioms.txt`](https://github.com/san-digital/comparator-opt/blob/main/lean/axioms.txt) and fails if any proof depends on `sorry`, `native_decide`, or a custom axiom. CI regenerates that file from the proofs on every run. Locally, `verify` only reads the committed copy; run [`lean/replay.sh`](https://github.com/san-digital/comparator-opt/blob/main/lean/replay.sh) (needs [elan](https://github.com/leanprover/elan)) to rebuild it.
3. The fast and reference functions must give the same output on every edge case and on 10,000 random inputs from a fixed seed. At least half of the random inputs must pass the validity check, so a validity check that rejects everything cannot make the comparison pass trivially. Then both are timed on the benchmark input.

After an intentional, reviewed change to a marked block, regenerate the lock with `cargo run --release -- lock .` and commit it with the change.

[`PROOF.md`](https://github.com/san-digital/comparator-opt/blob/main/PROOF.md) has the details of the Lean proofs and what they do and do not cover.

## What the lock protects

The lock is a file in the same repository, and `comparator-opt lock .` rewrites it. It does not stop anyone from changing the reference. If a marked block changes without a matching lock update, `verify` fails; with one, the change shows up in review as a change to `comparator.lock`. So the lock only helps if a person reviews every change to:

- `comparator.lock`, the marked blocks, and `lean/ComparatorOpt/Challenge.lean` and `Check.lean`;
- the rest of `src/kernels.rs`, not just the marked blocks. The lock checks the text between the markers, not what the compiler builds from it. For example, wrapping a marked block in a raw string literal and adding a different reference elsewhere in the file passes the lock;
- the fast functions themselves, for anything other than plain computation. They run in the same process as the checker, so a fast function that calls `std::process::exit(0)` ends `verify` early with a success status;
- `lean/ComparatorOpt/Proofs.lean`, because Lean can run code when it compiles a file, and CI compiles it;
- the code that runs the checks: `src/lock.rs`, `src/harness.rs`, `src/bin/comparator_opt.rs`, `lean/replay.sh`, `lean-toolchain`, `scripts/crosscheck.py`, and `.github/workflows/ci.yml`.

On GitHub, a [`CODEOWNERS`](https://github.com/san-digital/comparator-opt/blob/main/.github/CODEOWNERS) file plus branch protection with "Require review from Code Owners" turned on enforces this. CODEOWNERS on its own does not.

## Use it on your own code

The CLI only knows about this repository's kernels and files. To apply the pattern to your own functions, use the library from your tests:

```toml
[dev-dependencies]
comparator-opt = "0.1"
```

### 1. Describe the kernel and test it

Implement [`KernelSpec`](https://docs.rs/comparator-opt/latest/comparator_opt/harness/trait.KernelSpec.html) and call [`verify_kernel`](https://docs.rs/comparator-opt/latest/comparator_opt/harness/fn.verify_kernel.html) in a test:

```rust
use comparator_opt::{verify_kernel, KernelSpec, SplitMix64, VerifyConfig};

// The readable reference: count the set bits one at a time.
fn ref_popcount(x: u64) -> u32 {
    (0..64).filter(|i| (x >> i) & 1 == 1).count() as u32
}

// The fast version under test.
fn fast_popcount(x: u64) -> u32 {
    let x = x - ((x >> 1) & 0x5555_5555_5555_5555);
    let x = (x & 0x3333_3333_3333_3333) + ((x >> 2) & 0x3333_3333_3333_3333);
    let x = (x + (x >> 4)) & 0x0f0f_0f0f_0f0f_0f0f;
    (x.wrapping_mul(0x0101_0101_0101_0101) >> 56) as u32
}

struct Popcount;

impl KernelSpec for Popcount {
    type Input = u64;
    type Output = u32;
    fn name(&self) -> &'static str { "popcount" }
    fn is_valid(&self, _x: &u64) -> bool { true }
    fn eval_ref(&self, &x: &u64) -> u32 { ref_popcount(x) }
    fn eval_fast(&self, &x: &u64) -> u32 { fast_popcount(x) }
    fn boundary_inputs(&self) -> Vec<u64> { vec![0, 1, 1 << 63, u64::MAX] }
    fn sample_input(&self, rng: &mut SplitMix64) -> u64 { rng.next_u64() }
    fn bench_input(&self) -> u64 { 0x0123_4567_89ab_cdef }
}

let cfg = VerifyConfig { bench_iters: 100, ..VerifyConfig::default() };
let receipt = verify_kernel(&Popcount, &cfg).unwrap();
assert_eq!(receipt.sampled_valid, receipt.sampled_total);
```

### 2. Lock the reference

Put the reference function and the `KernelSpec` impl between `// @comparator:begin popcount` and `// @comparator:end popcount`, keep the fast function outside, and add a test that compares the blocks with a committed lock file:

```rust,no_run
use std::{fs, path::Path};
use comparator_opt::{build_lockfile, LockFile};

#[test]
fn pinned_reference_is_unchanged() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let actual = build_lockfile(root, &[("rust-ref", "src/popcount.rs")]).unwrap();
    let lock_path = root.join("comparator.lock");
    // Run once with UPDATE_COMPARATOR_LOCK=1 to create the lock, and again after any reviewed
    // change to a pinned block.
    if std::env::var_os("UPDATE_COMPARATOR_LOCK").is_some() {
        fs::write(&lock_path, actual.render()).unwrap();
    }
    let locked = LockFile::parse(&fs::read_to_string(&lock_path).unwrap()).unwrap();
    locked.verify_against(&actual).unwrap();
}
```

### 3. Protect the lock

Add a CODEOWNERS entry for the lock file, the file with the pinned blocks, and that test, and turn on code-owner review for your main branch (see [What the lock protects](#what-the-lock-protects)). Keeping fast functions in a separate file from the pinned blocks makes this easier to review.

### 4. Optional: Lean proofs

Copy [`lean/`](https://github.com/san-digital/comparator-opt/tree/main/lean), `lean-toolchain`, and the `lean` job in [`ci.yml`](https://github.com/san-digital/comparator-opt/blob/main/.github/workflows/ci.yml), replace the definitions and theorems with your own, update the theorem list in `replay.sh`, and call [`verify_lean_axiom_log`](https://docs.rs/comparator-opt/latest/comparator_opt/lock/fn.verify_lean_axiom_log.html) with your theorem names. Expect to write the Lean model of your fast path by hand; see [`PROOF.md`](https://github.com/san-digital/comparator-opt/blob/main/PROOF.md) for what that does and does not prove.

## Limits

- The differential tests try many inputs but do not prove the Rust fast path correct. A fast path that is wrong only on inputs the tests never try will pass.
- The Lean proofs are about Lean copies of the formulas, written out by hand in `Challenge.lean`, not about the Rust code. If someone rewrites a `fast_*` function, the proofs say nothing about the new code until a person updates the Lean copy (which changes the lock). The step from "correct for one byte" to "correct for 8 bytes packed in a `u64`" is argued in comments and tested, not proved.
- The speedups compare against the references exactly as written. They are deliberately simple and marked `#[inline(never)]`, which keeps the compiler from inlining or vectorizing them. Against plain inlined Rust the gap is much smaller, and sometimes reversed: on one Apple Silicon machine (rustc 1.99), a plain `iter().map(...).collect()` lowercase was faster than the SWAR version, a plain `filter(...).count()` was about 6x slower than the SWAR counter, and a plain `x % P` loop was about 2.5x to 3.5x slower than the fold. `x % (2^31 - 1)` with a constant divisor already compiles to a multiply and shifts, not a divide instruction.
- Timing is one run of each side, reference first, with no warm-up.
- The Mersenne kernel's test compares the sum of 64 results per batch, not each result.
- `utf8_char_count_swar` counts bytes that are not continuation bytes. That equals the number of characters only for valid UTF-8.

## Checks

The same checks CI runs:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
RUSTFLAGS="-D warnings" cargo test --release
cargo run --release -- verify .
python3 -I scripts/crosscheck.py
./lean/replay.sh && git diff --exit-code   # needs elan
```

See [`CONTRIBUTING.md`](https://github.com/san-digital/comparator-opt/blob/main/CONTRIBUTING.md) before opening a pull request, and [`SECURITY.md`](https://github.com/san-digital/comparator-opt/blob/main/SECURITY.md) to report a way past the checks.

## License

Apache-2.0. See [`LICENSE`](https://github.com/san-digital/comparator-opt/blob/main/LICENSE).
