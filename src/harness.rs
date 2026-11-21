use std::fmt;
use std::hint::black_box;
use std::time::Instant;

/// Deterministic 64-bit SplitMix64 PRNG for zero-dependency differential corpus generation.
#[derive(Clone, Debug)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    pub fn fill_bytes(&mut self, buf: &mut [u8]) {
        let mut chunks = buf.chunks_exact_mut(8);
        for c in &mut chunks {
            c.copy_from_slice(&self.next_u64().to_le_bytes());
        }
        let rem = chunks.into_remainder();
        if !rem.is_empty() {
            let w = self.next_u64().to_le_bytes();
            rem.copy_from_slice(&w[..rem.len()]);
        }
    }
}

/// Specification pairing a simple, human-readable reference implementation (`eval_ref`)
/// and input validity predicate (`is_valid`) with a candidate fast path (`eval_fast`).
pub trait KernelSpec {
    type Input: Clone + fmt::Debug;
    type Output: PartialEq + fmt::Debug;

    fn name(&self) -> &'static str;

    /// Domain guard `ValidInput(x)`. Pinned in `comparator.lock` and checked for non-vacuity.
    fn is_valid(&self, input: &Self::Input) -> bool;

    /// Crystal-clear reference implementation (`ref_fn`), pinned in `comparator.lock`.
    fn eval_ref(&self, input: &Self::Input) -> Self::Output;

    /// Candidate superoptimized implementation (`fast_fn`).
    fn eval_fast(&self, input: &Self::Input) -> Self::Output;

    /// Deterministic corner and cliff-edge inputs that must all satisfy `is_valid`.
    fn boundary_inputs(&self) -> Vec<Self::Input>;

    /// Deterministic corpus sampler used for non-vacuity and differential testing.
    fn sample_input(&self, rng: &mut SplitMix64) -> Self::Input;

    /// Representative benchmark input for measuring `eval_ref` vs `eval_fast` throughput.
    fn bench_input(&self) -> Self::Input;
}

/// Parameters controlling comparator verification and non-vacuity enforcement.
#[derive(Clone, Copy, Debug)]
pub struct VerifyConfig {
    pub samples: usize,
    pub seed: u64,
    /// Minimum required `is_valid` acceptance rate in basis points (`0..=10_000`).
    pub min_valid_bps: u32,
    pub bench_iters: u32,
}

impl Default for VerifyConfig {
    fn default() -> Self {
        Self {
            samples: 10_000,
            seed: 0x2026_1121_0000_0001,
            min_valid_bps: 5_000,
            bench_iters: 2_000,
        }
    }
}

/// Verification and performance receipt emitted when a `KernelSpec` passes all gates.
#[derive(Clone, Debug)]
pub struct VerifyReceipt {
    pub kernel: String,
    pub boundary_checked: usize,
    pub sampled_total: usize,
    pub sampled_valid: usize,
    pub valid_bps: u32,
    pub ref_ns_per_call: f64,
    pub fast_ns_per_call: f64,
    pub speedup_x: f64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HarnessError {
    BoundaryRejected {
        kernel: String,
        index: usize,
    },
    VacuousDomain {
        kernel: String,
        valid: usize,
        total: usize,
        min_bps: u32,
    },
    OutputMismatch {
        kernel: String,
        phase: &'static str,
        index: usize,
        detail: String,
    },
}

impl fmt::Display for HarnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BoundaryRejected { kernel, index } => {
                write!(f, "kernel '{kernel}': boundary input #{index} rejected by is_valid")
            }
            Self::VacuousDomain {
                kernel,
                valid,
                total,
                min_bps,
            } => write!(
                f,
                "kernel '{kernel}': domain guard accepted {valid}/{total} samples (below min {min_bps} bps)"
            ),
            Self::OutputMismatch {
                kernel,
                phase,
                index,
                detail,
            } => write!(
                f,
                "kernel '{kernel}': {phase} mismatch at #{index}: {detail}"
            ),
        }
    }
}

impl std::error::Error for HarnessError {}

/// Executes boundary, non-vacuity, differential equivalence, and timing checks on `spec`.
pub fn verify_kernel<K: KernelSpec>(
    spec: &K,
    cfg: &VerifyConfig,
) -> Result<VerifyReceipt, HarnessError> {
    let kname = spec.name().to_owned();

    // 1. Deterministic boundary vectors.
    let boundaries = spec.boundary_inputs();
    for (idx, inp) in boundaries.iter().enumerate() {
        if !spec.is_valid(inp) {
            return Err(HarnessError::BoundaryRejected {
                kernel: kname,
                index: idx,
            });
        }
        let expected = spec.eval_ref(inp);
        let got = spec.eval_fast(inp);
        if got != expected {
            return Err(HarnessError::OutputMismatch {
                kernel: kname,
                phase: "boundary",
                index: idx,
                detail: format!("expected={expected:?}, got={got:?}"),
            });
        }
    }

    // 2. Sampled corpus non-vacuity and differential check.
    let mut rng = SplitMix64::new(cfg.seed);
    let mut valid_count = 0usize;
    for idx in 0..cfg.samples {
        let inp = spec.sample_input(&mut rng);
        if !spec.is_valid(&inp) {
            continue;
        }
        valid_count += 1;
        let expected = spec.eval_ref(&inp);
        let got = spec.eval_fast(&inp);
        if got != expected {
            return Err(HarnessError::OutputMismatch {
                kernel: kname,
                phase: "differential",
                index: idx,
                detail: format!("expected={expected:?}, got={got:?}"),
            });
        }
    }

    let valid_bps = if cfg.samples == 0 {
        10_000
    } else {
        ((valid_count as u128 * 10_000) / (cfg.samples as u128)) as u32
    };
    if cfg.samples > 0 && valid_bps < cfg.min_valid_bps {
        return Err(HarnessError::VacuousDomain {
            kernel: kname,
            valid: valid_count,
            total: cfg.samples,
            min_bps: cfg.min_valid_bps,
        });
    }

    // 3. Calibrated black-box timing comparison.
    let bench_inp = spec.bench_input();
    assert!(spec.is_valid(&bench_inp));
    assert_eq!(spec.eval_ref(&bench_inp), spec.eval_fast(&bench_inp));

    let iters = cfg.bench_iters.max(1);
    let t_ref = Instant::now();
    for _ in 0..iters {
        black_box(spec.eval_ref(black_box(&bench_inp)));
    }
    let dt_ref = t_ref.elapsed().as_nanos() as f64;

    let t_fast = Instant::now();
    for _ in 0..iters {
        black_box(spec.eval_fast(black_box(&bench_inp)));
    }
    let dt_fast = t_fast.elapsed().as_nanos() as f64;

    let ref_ns = dt_ref / (iters as f64);
    let fast_ns = (dt_fast / (iters as f64)).max(0.01);
    let speedup_x = ref_ns / fast_ns;

    Ok(VerifyReceipt {
        kernel: kname,
        boundary_checked: boundaries.len(),
        sampled_total: cfg.samples,
        sampled_valid: valid_count,
        valid_bps,
        ref_ns_per_call: ref_ns,
        fast_ns_per_call: fast_ns,
        speedup_x,
    })
}
