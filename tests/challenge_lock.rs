use std::fs;
use std::path::PathBuf;

use comparator_opt::{
    build_lockfile, extract_pins_from_str, fast_mersenne31_mod, ref_mersenne31_mod,
    ref_mersenne31_valid, ref_utf8_char_count, verify_kernel, verify_lean_axiom_log, HarnessError,
    KernelSpec, LockError, LockFile, SplitMix64, Utf8CountKernel, VerifyConfig, PIN_TARGETS,
    REQUIRED_THEOREMS,
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn locked() -> LockFile {
    LockFile::parse(&fs::read_to_string(root().join("comparator.lock")).unwrap()).unwrap()
}

/// Builds a lockfile from the real pin targets, substituting `kernels_src` for `src/kernels.rs`.
fn lock_with_kernels(kernels_src: &str) -> LockFile {
    let mut pins = Vec::new();
    for &(kind, rel) in PIN_TARGETS {
        let content = if rel == "src/kernels.rs" {
            kernels_src.to_owned()
        } else {
            fs::read_to_string(root().join(rel)).unwrap()
        };
        pins.extend(extract_pins_from_str(kind, rel, &content).unwrap());
    }
    LockFile { pins }
}

fn expect_digest_mismatch(tampered: &str, expected_id: &str) {
    match locked().verify_against(&lock_with_kernels(tampered)) {
        Err(LockError::DigestMismatch { id, .. }) => assert_eq!(id, expected_id),
        other => panic!("expected DigestMismatch on {expected_id}, got {other:?}"),
    }
}

#[test]
fn challenge_lock_accepts_repo_and_rejects_reference_tampering() {
    let lock = locked();
    assert_eq!(lock.pins.len(), 7);
    let actual = build_lockfile(&root(), PIN_TARGETS).unwrap();
    assert_eq!(lock.verify_against(&actual), Ok(()));

    let kernels_src = fs::read_to_string(root().join("src/kernels.rs")).unwrap();

    // 1. Tampering with the reference function itself (`b + 32` -> `b + 31`).
    let tampered = kernels_src.replacen("b + 32", "b + 31", 1);
    assert_ne!(kernels_src, tampered);
    expect_digest_mismatch(&tampered, "ascii_lower_ref");

    // 2. Rewiring `eval_ref` to the fast path, so the differential suite compares the fast
    //    path with itself. The `KernelSpec` impl now sits inside the pinned block.
    let tampered = kernels_src.replacen(
        "        ref_ascii_lower(input)\n",
        "        fast_ascii_lower_swar(input)\n",
        1,
    );
    assert_ne!(kernels_src, tampered);
    expect_digest_mismatch(&tampered, "ascii_lower_ref");

    // 3. Widening the validity guard of the Mersenne kernel.
    let tampered =
        kernels_src.replacen("input.iter().copied().all(ref_mersenne31_valid)", "true", 1);
    assert_ne!(kernels_src, tampered);
    expect_digest_mismatch(&tampered, "mersenne31_mod_ref");

    // 4. Shrinking the sampled corpus so it never exercises long inputs.
    let tampered = kernels_src.replacen("% 600", "% 8", 1);
    assert_ne!(kernels_src, tampered);
    expect_digest_mismatch(&tampered, "utf8_count_ref");
}

#[test]
fn axiom_gate_rejects_sorry_native_decide_and_custom_axioms() {
    let real_log = fs::read_to_string(root().join("lean/axioms.txt")).unwrap();
    let ok = verify_lean_axiom_log(&real_log, REQUIRED_THEOREMS).unwrap();
    assert_eq!(ok.len(), 10);

    // 1. Reject `sorryAx`.
    let sorry_log =
        "'ComparatorOpt.ascii_lower_byte_sound' depends on axioms: [propext, sorryAx]\n";
    assert_eq!(
        verify_lean_axiom_log(sorry_log, &["ComparatorOpt.ascii_lower_byte_sound"]),
        Err(LockError::DisallowedAxiom {
            theorem: "ComparatorOpt.ascii_lower_byte_sound".to_owned(),
            axiom: "sorryAx".to_owned(),
        })
    );

    // 2. Reject `Lean.ofReduceBool` (`native_decide`).
    let native_log =
        "'ComparatorOpt.utf8_lead_bit_sound' depends on axioms: [propext, Lean.ofReduceBool]\n";
    assert_eq!(
        verify_lean_axiom_log(native_log, &["ComparatorOpt.utf8_lead_bit_sound"]),
        Err(LockError::DisallowedAxiom {
            theorem: "ComparatorOpt.utf8_lead_bit_sound".to_owned(),
            axiom: "Lean.ofReduceBool".to_owned(),
        })
    );

    // 3. Reject a custom `axiom` declaration.
    let custom_log =
        "'ComparatorOpt.mersenne31_fast_sound' depends on axioms: [propext, ComparatorOpt.cheat]\n";
    assert_eq!(
        verify_lean_axiom_log(custom_log, &["ComparatorOpt.mersenne31_fast_sound"]),
        Err(LockError::DisallowedAxiom {
            theorem: "ComparatorOpt.mersenne31_fast_sound".to_owned(),
            axiom: "ComparatorOpt.cheat".to_owned(),
        })
    );

    // 4. Reject missing theorem audit entry.
    assert_eq!(
        verify_lean_axiom_log(&real_log, &["ComparatorOpt.missing_theorem"]),
        Err(LockError::MissingAxiomAudit {
            theorem: "ComparatorOpt.missing_theorem".to_owned(),
        })
    );
}

#[test]
fn axiom_gate_reads_lists_that_lean_wraps_across_lines() {
    // Verbatim `#print axioms` output from Lean 4 for a long footprint: the list wraps, and
    // every axiom after `propext` sits on a continuation line.
    let wrapped_log = "\
'ComparatorOpt.horizontal_byte_sum_sound' depends on axioms: [propext,
 sorryAx,
 Classical.choice,
 ComparatorOpt.a_rather_long_custom_axiom_name_for_testing_wrap,
 Quot.sound]
";
    assert_eq!(
        verify_lean_axiom_log(wrapped_log, &["ComparatorOpt.horizontal_byte_sum_sound"]),
        Err(LockError::DisallowedAxiom {
            theorem: "ComparatorOpt.horizontal_byte_sum_sound".to_owned(),
            axiom: "sorryAx".to_owned(),
        })
    );

    let clean_wrapped = "\
'ComparatorOpt.horizontal_byte_sum_sound' depends on axioms: [propext,
 Classical.choice,
 Quot.sound]
";
    let ok =
        verify_lean_axiom_log(clean_wrapped, &["ComparatorOpt.horizontal_byte_sum_sound"]).unwrap();
    assert_eq!(
        ok["ComparatorOpt.horizontal_byte_sum_sound"],
        ["propext", "Classical.choice", "Quot.sound"]
    );

    let truncated = "'ComparatorOpt.horizontal_byte_sum_sound' depends on axioms: [propext,\n";
    assert!(matches!(
        verify_lean_axiom_log(truncated, &["ComparatorOpt.horizontal_byte_sum_sound"]),
        Err(LockError::MalformedLock { .. })
    ));
}

#[test]
fn harness_rejects_vacuous_domain_and_buggy_255_word_overflow() {
    struct VacuousKernel;
    impl KernelSpec for VacuousKernel {
        type Input = u64;
        type Output = u64;
        fn name(&self) -> &'static str {
            "vacuous_demo"
        }
        fn is_valid(&self, &x: &u64) -> bool {
            x == 0
        }
        fn eval_ref(&self, &x: &u64) -> u64 {
            x
        }
        fn eval_fast(&self, &x: &u64) -> u64 {
            999 * x
        }
        fn boundary_inputs(&self) -> Vec<u64> {
            vec![0]
        }
        fn sample_input(&self, rng: &mut SplitMix64) -> u64 {
            rng.next_u64() | 1
        }
        fn bench_input(&self) -> u64 {
            0
        }
    }

    let res = verify_kernel(&VacuousKernel, &VerifyConfig::default());
    assert!(matches!(res, Err(HarnessError::VacuousDomain { .. })));

    // Naive SWAR counter that accumulates every word before one horizontal sum: once a lane
    // total reaches 256 the top byte of `acc * 0x0101_0101_0101_0101` wraps to zero.
    fn naive_utf8_count(bytes: &[u8]) -> usize {
        let words = bytes.chunks_exact(8);
        let rem = words.remainder();
        let mut acc = 0u64;
        for src in words {
            let w = u64::from_le_bytes(src.try_into().unwrap());
            acc = acc.wrapping_add((((!w) | (w << 1)) & 0x8080_8080_8080_8080) >> 7);
        }
        let mut total = (acc.wrapping_mul(0x0101_0101_0101_0101) >> 56) as usize;
        for &b in rem {
            total += ((((!b) | (b << 1)) & 0x80) >> 7) as usize;
        }
        total
    }

    struct NaiveUtf8Kernel;
    impl KernelSpec for NaiveUtf8Kernel {
        type Input = Vec<u8>;
        type Output = usize;
        fn name(&self) -> &'static str {
            "naive_utf8_count"
        }
        fn is_valid(&self, input: &Vec<u8>) -> bool {
            Utf8CountKernel.is_valid(input)
        }
        fn eval_ref(&self, input: &Vec<u8>) -> usize {
            ref_utf8_char_count(input)
        }
        fn eval_fast(&self, input: &Vec<u8>) -> usize {
            naive_utf8_count(input)
        }
        fn boundary_inputs(&self) -> Vec<Vec<u8>> {
            Utf8CountKernel.boundary_inputs()
        }
        fn sample_input(&self, rng: &mut SplitMix64) -> Vec<u8> {
            Utf8CountKernel.sample_input(rng)
        }
        fn bench_input(&self) -> Vec<u8> {
            Utf8CountKernel.bench_input()
        }
    }

    // The 256-byte all-lead boundary vector exposes the wrap (256 counted as 0).
    match verify_kernel(&NaiveUtf8Kernel, &VerifyConfig::default()) {
        Err(HarnessError::OutputMismatch { phase, index, .. }) => {
            assert_eq!(phase, "boundary");
            assert_eq!(Utf8CountKernel.boundary_inputs()[index].len(), 256);
        }
        other => panic!("expected boundary OutputMismatch, got {other:?}"),
    }

    // Demonstrate why `ref_mersenne31_valid` (`x < (2^31 - 1)^2`) is non-trivial:
    // at `u64::MAX` (outside ValidInput), two 31-bit folds leave `u >= 2^31`.
    assert!(!ref_mersenne31_valid(u64::MAX));
    assert_ne!(fast_mersenne31_mod(u64::MAX), ref_mersenne31_mod(u64::MAX));
}

#[test]
fn block_parser_handles_crlf_nesting_duplicates_and_stray_markers() {
    let lf = "// @comparator:begin a\nfn a() {}\n// @comparator:end a\n";
    let crlf = lf.replace('\n', "\r\n");
    let lf_pins = extract_pins_from_str("rust-ref", "x.rs", lf).unwrap();
    let crlf_pins = extract_pins_from_str("rust-ref", "x.rs", &crlf).unwrap();
    assert_eq!(lf_pins.len(), 1);
    // CRLF checkouts hash the same as LF checkouts.
    assert_eq!(lf_pins, crlf_pins);

    // Lean-style markers are recognized too.
    let lean = "-- @comparator:begin b\ndef b := 1\n-- @comparator:end b\n";
    assert_eq!(
        extract_pins_from_str("lean-challenge", "x.lean", lean)
            .unwrap()
            .len(),
        1
    );

    let nested = "// @comparator:begin a\n// @comparator:begin b\n// @comparator:end b\n// @comparator:end a\n";
    assert!(matches!(
        extract_pins_from_str("rust-ref", "x.rs", nested),
        Err(LockError::UnclosedMarker { id, .. }) if id == "a"
    ));

    let duplicate = format!("{lf}{lf}");
    assert!(matches!(
        extract_pins_from_str("rust-ref", "x.rs", &duplicate),
        Err(LockError::DuplicateId { id }) if id == "a"
    ));

    let unclosed = "// @comparator:begin a\nfn a() {}\n";
    assert!(matches!(
        extract_pins_from_str("rust-ref", "x.rs", unclosed),
        Err(LockError::UnclosedMarker { .. })
    ));

    let stray_end = "fn a() {}\n// @comparator:end a\n";
    assert!(matches!(
        extract_pins_from_str("rust-ref", "x.rs", stray_end),
        Err(LockError::MalformedLock { .. })
    ));

    let mismatched = "// @comparator:begin a\n// @comparator:end b\n";
    assert!(matches!(
        extract_pins_from_str("rust-ref", "x.rs", mismatched),
        Err(LockError::UnclosedMarker { id, .. }) if id == "a"
    ));
}
