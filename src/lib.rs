#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

pub mod harness;
pub mod kernels;
pub mod lock;
pub mod sha256;

pub use harness::{
    verify_kernel, HarnessError, KernelSpec, SplitMix64, VerifyConfig, VerifyReceipt,
};
pub use kernels::{
    fast_ascii_lower_swar, fast_mersenne31_mod, fast_utf8_char_count_swar, ref_ascii_lower,
    ref_ascii_lower_byte, ref_is_utf8_lead, ref_mersenne31_mod, ref_mersenne31_valid,
    ref_utf8_char_count, swar_ascii_lower_u64, AsciiLowerKernel, Mersenne31BatchKernel,
    Utf8CountKernel, MERSENNE31,
};
pub use lock::{
    build_lockfile, extract_pins_from_str, verify_lean_axiom_log, ChallengePin, LockError,
    LockFile, ALLOWED_LEAN_AXIOMS,
};
pub use sha256::{sha256, sha256_hex};
