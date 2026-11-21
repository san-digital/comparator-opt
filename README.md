# comparator-opt

Zero-trust superoptimization harness and cryptographic challenge lock for pairing short, readable reference functions with verified bit-twiddling fast paths.

Zero runtime dependencies. `#![forbid(unsafe_code)]`. Stock Lean 4 proofs (`Init` only, replayed via `leanchecker` with zero `sorryAx` and zero `native_decide`) in [`lean/ComparatorOpt/`](https://github.com/san-digital/comparator-opt/tree/main/lean/ComparatorOpt). Architecture details in [`PROOF.md`](https://github.com/san-digital/comparator-opt/blob/main/PROOF.md).

## Installation

```sh
cargo install comparator-opt
```

Or as a library dependency:

```toml
[dependencies]
comparator-opt = "0.1"
```

## Why This Exists

Automated optimizers and coding agents can turn a 6-line scalar loop into a 10x-25x faster branchless SWAR or division-free kernel in seconds, yet engineering teams hesitate to merge unreadable bit-twiddling:

1. **Human review bottleneck:** Reviewing 64-bit carry-mask arithmetic by eye misses subtle lane-overflow bugs (for example, accumulating 255 `u64` words before multiplying by `0x0101_0101_0101_0101 >> 56` overflows the top byte as soon as the byte sum hits `256`).
2. **Specification drift:** If an optimizer edits both the fast path and the test/reference function (or narrows the input precondition `ValidInput` to an empty set), CI passes vacuously.

`comparator-opt` adapts the **Challenge + Comparator** verification split to software kernels:

- **Humans maintain only the trivial reference specification:** Short `ref_*` functions, `ValidInput` guards, and Lean theorem statements sit inside `@comparator:begin <id>` .. `@comparator:end <id>` markers locked by FIPS 180-4 SHA-256 digests in [`comparator.lock`](https://github.com/san-digital/comparator-opt/blob/main/comparator.lock).
- **Optimizers supply the fast path and proof:** Any edit to a locked reference block, any disallowed Lean axiom (`sorryAx`, `Lean.ofReduceBool`), any vacuous `ValidInput` guard, or any differential mismatch fails `comparator-opt verify`.

## Measured Speedups (Built-In Verified Kernels)

Single-core `cargo run --release -- verify .` on Apple Silicon (M-series, `rustc 1.94.0`):

| Kernel | Reference Spec | Verified Fast Path | Corpus Valid Rate | Ref Latency | Fast Latency | Speedup |
| :--- | :--- | :--- | :--- | ---: | ---: | ---: |
| `ascii_lower_swar` (16 KB HTTP headers) | 6-line byte range check | Carry-free 8-lane `u64` SWAR (`0x3f` / `0x25` MSB masks) | 10,000 / 10,000 | 13,859 ns | 1,231 ns | **11.3x** |
| `utf8_char_count_swar` (16.5 KB UTF-8) | `(b & 0xc0) != 0x80` loop | 31-word (`248`-byte) blocked SWAR + `* 0x0101... >> 56` | 10,000 / 10,000 | 17,221 ns | 644 ns | **26.7x** |
| `mersenne31_mod_fold` (4,096 products `< P^2`) | Hardware `x % (2^31 - 1)` | Division-free two-round 31-bit fold + mask | 8,757 / 10,000 | 4,626 ns | 1,175 ns | **3.9x** |

## CLI Usage

Verify the cryptographic Challenge Lock, Lean 4 axiom footprint, non-vacuity ratio, boundary vectors, and differential suite:

```sh
cargo run --release -- verify .
```

Output:

```text
[1/3] challenge lock verified (6 pinned blocks match SHA-256)
[2/3] Lean 4 axiom footprint verified (5 theorems, zero sorryAx / native_decide)
[3/3] running boundary, non-vacuity, and differential equivalence harness:
  kernel=ascii_lower_swar       valid=10000/10000 (10000 bps) ref= 13859.3 ns  fast=  1231.4 ns  speedup=11.25x
  kernel=utf8_char_count_swar   valid=10000/10000 (10000 bps) ref= 17220.8 ns  fast=   643.9 ns  speedup=26.74x
  kernel=mersenne31_mod_fold    valid=8757/10000 (8757 bps) ref=  4626.3 ns  fast=  1174.6 ns  speedup= 3.94x
```

Regenerate `comparator.lock` after an intentional human edit to a reference block:

```sh
cargo run --release -- lock .
```

## Library Usage

```rust
use comparator_opt::{
    fast_ascii_lower_swar, fast_mersenne31_mod, fast_utf8_char_count_swar, ref_ascii_lower,
    ref_mersenne31_mod, ref_mersenne31_valid, ref_utf8_char_count, MERSENNE31,
};

let hdr = b"Content-Type: Application/JSON\r\n";
assert_eq!(fast_ascii_lower_swar(hdr), ref_ascii_lower(hdr));

let text = "ZMod p and F_p^x".as_bytes();
assert_eq!(fast_utf8_char_count_swar(text), ref_utf8_char_count(text));

let prod = (MERSENNE31 - 5) * (MERSENNE31 - 2);
assert!(ref_mersenne31_valid(prod));
assert_eq!(fast_mersenne31_mod(prod), ref_mersenne31_mod(prod));
```

## Verification

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
RUSTFLAGS="-D warnings" cargo test --release
cargo run --release -- verify .
python3 scripts/crosscheck.py
```

To recompile and kernel-replay the Lean 4 proofs from scratch (requires stock Lean 4 only):

```sh
./lean/replay.sh
```

## License

Apache-2.0. See [`LICENSE`](https://github.com/san-digital/comparator-opt/blob/main/LICENSE).
