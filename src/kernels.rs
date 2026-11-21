//! The three example kernels. Each has a readable `ref_*` function and a `KernelSpec` impl
//! inside a pinned `@comparator` block, and a `fast_*` function outside it.

use crate::harness::{KernelSpec, SplitMix64};

// Each `@comparator` block below pins a kernel's reference function together with its
// `KernelSpec` wiring: the validity guard, which function `eval_ref` calls, the boundary
// vectors, the sampled corpus, and the benchmark input. `eval_fast` only delegates to the
// unpinned `fast_*` function after the block, which is the part meant to change. Changes to
// anything else in this file still need review: code outside a block can change what the
// compiler builds from it (see "What the lock protects" in the README).

// ============================================================================
// Kernel 1: ASCII lowercase, 8 bytes at a time (SWAR)
// ============================================================================

// @comparator:begin ascii_lower_ref
#[inline(never)]
pub fn ref_ascii_lower_byte(b: u8) -> u8 {
    if b.is_ascii_uppercase() {
        b + 32
    } else {
        b
    }
}

#[inline(never)]
pub fn ref_ascii_lower(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    for &b in input {
        out.push(ref_ascii_lower_byte(b));
    }
    out
}

pub struct AsciiLowerKernel;

impl KernelSpec for AsciiLowerKernel {
    type Input = Vec<u8>;
    type Output = Vec<u8>;

    fn name(&self) -> &'static str {
        "ascii_lower_swar"
    }

    fn is_valid(&self, _input: &Self::Input) -> bool {
        true
    }

    fn eval_ref(&self, input: &Self::Input) -> Self::Output {
        ref_ascii_lower(input)
    }

    fn eval_fast(&self, input: &Self::Input) -> Self::Output {
        fast_ascii_lower_swar(input)
    }

    fn boundary_inputs(&self) -> Vec<Self::Input> {
        let all_bytes: Vec<u8> = (0u8..=255).collect();
        vec![
            vec![],
            b"@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{".to_vec(),
            all_bytes,
            vec![0x40, 0x41, 0x5a, 0x5b, 0xc0, 0xc1, 0xda, 0xdb, 0xff],
        ]
    }

    fn sample_input(&self, rng: &mut SplitMix64) -> Self::Input {
        let len = (rng.next_u64() as usize) % 97;
        let mut buf = vec![0u8; len];
        rng.fill_bytes(&mut buf);
        buf
    }

    fn bench_input(&self) -> Self::Input {
        let mut v = Vec::with_capacity(16_384);
        let sample = b"Content-Type: Application/JSON; X-Request-ID: ABCDEF0123456789\r\n";
        while v.len() < 16_384 {
            v.extend_from_slice(sample);
        }
        v.truncate(16_384);
        v
    }
}
// @comparator:end ascii_lower_ref

/// Lowercases the ASCII letters in all 8 bytes of a `u64` at once. Other bytes are unchanged.
///
/// `lean/ComparatorOpt/Proofs.lean` proves the per-lane formula exact for all 256 byte values
/// (`ascii_lower_byte_sound`). Lane independence in the packed word rests on the carry-free
/// argument below, which is checked by tests on every byte in every lane position and is not
/// itself formalized in Lean:
/// - `low7 = w & 0x7f7f_7f7f_7f7f_7f7f` clears bit 7 so per-lane sums stay `<= 190 < 256`
///   and never carry into neighboring lanes.
/// - `ge_a = low7 + 0x3f3f_3f3f_3f3f_3f3f` sets bit 7 iff `(b & 0x7f) >= 65` (`'A'`).
/// - `gt_z = low7 + 0x2525_2525_2525_2525` sets bit 7 iff `(b & 0x7f) >= 91` (`'Z' + 1`).
/// - Masking with `!w & 0x8080_8080_8080_8080` excludes non-ASCII bytes (`b >= 128`).
#[inline]
pub fn swar_ascii_lower_u64(w: u64) -> u64 {
    const LOW7: u64 = 0x7f7f_7f7f_7f7f_7f7f;
    const MSB: u64 = 0x8080_8080_8080_8080;
    let low7 = w & LOW7;
    let ge_a = low7.wrapping_add(0x3f3f_3f3f_3f3f_3f3f);
    let gt_z = low7.wrapping_add(0x2525_2525_2525_2525);
    let upper_msb = ge_a & !gt_z & !w & MSB;
    w | (upper_msb >> 2)
}

/// Fast version of [`ref_ascii_lower`]: applies [`swar_ascii_lower_u64`] to each 8-byte chunk.
#[inline(never)]
pub fn fast_ascii_lower_swar(input: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; input.len()];
    let mut in_chunks = input.chunks_exact(8);
    let mut out_chunks = out.chunks_exact_mut(8);
    for (src, dst) in (&mut in_chunks).zip(&mut out_chunks) {
        let w = u64::from_le_bytes([
            src[0], src[1], src[2], src[3], src[4], src[5], src[6], src[7],
        ]);
        dst.copy_from_slice(&swar_ascii_lower_u64(w).to_le_bytes());
    }
    for (&b, dst) in in_chunks
        .remainder()
        .iter()
        .zip(out_chunks.into_remainder().iter_mut())
    {
        *dst = swar_ascii_lower_u64(b as u64) as u8;
    }
    out
}

// ============================================================================
// Kernel 2: count UTF-8 characters (non-continuation bytes), 8 bytes at a time
// ============================================================================

// @comparator:begin utf8_count_ref
#[inline(never)]
pub fn ref_is_utf8_lead(b: u8) -> bool {
    (b & 0xc0) != 0x80
}

#[inline(never)]
pub fn ref_utf8_char_count(bytes: &[u8]) -> usize {
    let mut count = 0usize;
    for &b in bytes {
        if ref_is_utf8_lead(b) {
            count += 1;
        }
    }
    count
}

pub struct Utf8CountKernel;

impl KernelSpec for Utf8CountKernel {
    type Input = Vec<u8>;
    type Output = usize;

    fn name(&self) -> &'static str {
        "utf8_char_count_swar"
    }

    fn is_valid(&self, _input: &Self::Input) -> bool {
        true
    }

    fn eval_ref(&self, input: &Self::Input) -> Self::Output {
        ref_utf8_char_count(input)
    }

    fn eval_fast(&self, input: &Self::Input) -> Self::Output {
        fast_utf8_char_count_swar(input)
    }

    fn boundary_inputs(&self) -> Vec<Self::Input> {
        let all_bytes: Vec<u8> = (0u8..=255).collect();
        vec![
            vec![],
            vec![0x00; 248], // Exact 31-word block full of lead bytes (count = 248)
            vec![0x00; 256], // Crosses 31-word block boundary (count = 256, catches naive 255-word bug)
            vec![0x80; 260], // All continuation bytes (count = 0)
            vec![0xbf; 260], // All continuation bytes at upper edge 0b1011_1111 (count = 0)
            vec![0xc0; 260], // All 2-byte lead bytes 0b1100_0000 (count = 260)
            all_bytes,
        ]
    }

    fn sample_input(&self, rng: &mut SplitMix64) -> Self::Input {
        let len = (rng.next_u64() as usize) % 600;
        let mut buf = vec![0u8; len];
        rng.fill_bytes(&mut buf);
        buf
    }

    fn bench_input(&self) -> Self::Input {
        "Hello, UTF-8 token stream: cafe, resume, naive, ZMod, F_p! "
            .as_bytes()
            .repeat(280)
    }
}
// @comparator:end utf8_count_ref

/// Fast version of [`ref_utf8_char_count`]: counts bytes that are not UTF-8 continuation bytes
/// (`0b10xx_xxxx`), 8 at a time. For valid UTF-8 this is the number of characters.
///
/// Within each block of at most `31` `u64` words (`31 * 8 = 248 < 256` bytes), the sum
/// across all 8 byte accumulators never exceeds `248`, so multiplying by `0x0101_0101_0101_0101`
/// and shifting right by `56` recovers the exact block count without overflow.
#[inline(never)]
pub fn fast_utf8_char_count_swar(bytes: &[u8]) -> usize {
    const MSB: u64 = 0x8080_8080_8080_8080;
    const ONES: u64 = 0x0101_0101_0101_0101;

    let mut total = 0usize;
    let blocks = bytes.chunks_exact(31 * 8);
    let after_blocks = blocks.remainder();

    for block in blocks {
        let mut safe_acc = 0u64;
        for src in block.chunks_exact(8) {
            let w = u64::from_le_bytes([
                src[0], src[1], src[2], src[3], src[4], src[5], src[6], src[7],
            ]);
            safe_acc = safe_acc.wrapping_add((((!w) | (w << 1)) & MSB) >> 7);
        }
        total += ((safe_acc.wrapping_mul(ONES)) >> 56) as usize;
    }

    let tail_words = after_blocks.chunks_exact(8);
    let rem = tail_words.remainder();
    let mut tail_acc = 0u64;
    for src in tail_words {
        let w = u64::from_le_bytes([
            src[0], src[1], src[2], src[3], src[4], src[5], src[6], src[7],
        ]);
        tail_acc = tail_acc.wrapping_add((((!w) | (w << 1)) & MSB) >> 7);
    }
    total += ((tail_acc.wrapping_mul(ONES)) >> 56) as usize;

    for &b in rem {
        total += ((((!b) | (b << 1)) & 0x80) >> 7) as usize;
    }
    total
}

// ============================================================================
// Kernel 3: reduce a product modulo the Mersenne prime 2^31 - 1 without `%`
// ============================================================================

// @comparator:begin mersenne31_mod_ref
pub const MERSENNE31: u64 = 0x7fff_ffff;

#[inline(never)]
pub fn ref_mersenne31_valid(x: u64) -> bool {
    x < MERSENNE31 * MERSENNE31
}

#[inline(never)]
pub fn ref_mersenne31_mod(x: u64) -> u32 {
    (x % MERSENNE31) as u32
}

pub struct Mersenne31BatchKernel;

impl KernelSpec for Mersenne31BatchKernel {
    type Input = Vec<u64>;
    type Output = u64;

    fn name(&self) -> &'static str {
        "mersenne31_mod_fold"
    }

    fn is_valid(&self, input: &Self::Input) -> bool {
        input.iter().copied().all(ref_mersenne31_valid)
    }

    fn eval_ref(&self, input: &Self::Input) -> Self::Output {
        let mut acc = 0u64;
        for &x in input {
            acc = acc.wrapping_add(ref_mersenne31_mod(x) as u64);
        }
        acc
    }

    fn eval_fast(&self, input: &Self::Input) -> Self::Output {
        fast_mersenne31_batch(input)
    }

    fn boundary_inputs(&self) -> Vec<Self::Input> {
        let p = MERSENNE31;
        let max_valid = p * p - 1;
        vec![vec![
            0,
            1,
            p - 1,
            p,
            p + 1,
            2 * p - 1,
            2 * p,
            (1u64 << 31) - 1,
            1u64 << 31,
            (1u64 << 32) - 4,
            max_valid - p,
            max_valid - 1,
            max_valid,
        ]]
    }

    fn sample_input(&self, rng: &mut SplitMix64) -> Self::Input {
        let out_of_domain_batch = (rng.next_u64() & 0x7) == 0;
        let mut out = Vec::with_capacity(64);
        for _ in 0..64 {
            if out_of_domain_batch {
                out.push(rng.next_u64());
            } else {
                let a = rng.next_u64() % MERSENNE31;
                let b = rng.next_u64() % MERSENNE31;
                out.push(a * b);
            }
        }
        out
    }

    fn bench_input(&self) -> Self::Input {
        let mut rng = SplitMix64::new(0x517c_c1b7_2722_0a95);
        let p2 = MERSENNE31 * MERSENNE31;
        (0..4_096).map(|_| rng.next_u64() % p2).collect()
    }
}
// @comparator:end mersenne31_mod_ref

/// Computes `x % (2^31 - 1)` with two shift-and-add folds and no division, for
/// `x < (2^31 - 1)^2` (any product of two reduced values). Outside that range the result can be
/// wrong.
///
/// `lean/ComparatorOpt/Proofs.lean` proves the same formula, written in Lean over `Nat`, exact
/// on that range. The proof is about the Lean copy, not this function.
#[inline]
pub fn fast_mersenne31_mod(x: u64) -> u32 {
    const P: u64 = 0x7fff_ffff;
    let t = (x & P) + (x >> 31);
    let u = (t & P) + (t >> 31);
    let mask = ((u == P) as u64).wrapping_neg();
    (u & !mask) as u32
}

/// Batched fast path for `Mersenne31BatchKernel`: wrapping sum of `fast_mersenne31_mod`.
#[inline(never)]
pub fn fast_mersenne31_batch(input: &[u64]) -> u64 {
    let mut acc = 0u64;
    for &x in input {
        acc = acc.wrapping_add(fast_mersenne31_mod(x) as u64);
    }
    acc
}
