use std::fs;
use std::path::PathBuf;

use comparator_opt::{
    build_lockfile, extract_pins_from_str, fast_mersenne31_mod, ref_mersenne31_mod,
    ref_mersenne31_valid, verify_kernel, verify_lean_axiom_log, HarnessError, KernelSpec,
    LockError, LockFile, SplitMix64, VerifyConfig,
};

#[test]
fn challenge_lock_accepts_repo_and_rejects_reference_tampering() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lock_text = fs::read_to_string(root.join("comparator.lock")).unwrap();
    let locked = LockFile::parse(&lock_text).unwrap();
    assert_eq!(locked.pins.len(), 6);

    let actual = build_lockfile(
        &root,
        &[
            ("rust-ref", "src/kernels.rs"),
            ("lean-challenge", "lean/ComparatorOpt/Challenge.lean"),
        ],
    )
    .unwrap();
    assert_eq!(locked.verify_against(&actual), Ok(()));

    // Simulate an optimizer tampering with the reference spec (`b + 32` -> `b + 31`).
    let kernels_src = fs::read_to_string(root.join("src/kernels.rs")).unwrap();
    let tampered_src = kernels_src.replacen("b + 32", "b + 31", 1);
    assert_ne!(kernels_src, tampered_src);
    let mut tampered_pins =
        extract_pins_from_str("rust-ref", "src/kernels.rs", &tampered_src).unwrap();
    let challenge_src = fs::read_to_string(root.join("lean/ComparatorOpt/Challenge.lean")).unwrap();
    tampered_pins.extend(
        extract_pins_from_str(
            "lean-challenge",
            "lean/ComparatorOpt/Challenge.lean",
            &challenge_src,
        )
        .unwrap(),
    );
    let tampered_lock = LockFile {
        pins: tampered_pins,
    };

    match locked.verify_against(&tampered_lock) {
        Err(LockError::DigestMismatch { id, .. }) => assert_eq!(id, "ascii_lower_ref"),
        other => panic!("expected DigestMismatch on ascii_lower_ref, got {other:?}"),
    }
}

#[test]
fn axiom_gate_rejects_sorry_native_decide_and_custom_axioms() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let real_log = fs::read_to_string(root.join("lean/axioms.txt")).unwrap();
    let required = &[
        "ComparatorOpt.ascii_lower_byte_sound",
        "ComparatorOpt.swar_no_carry_add_sound",
        "ComparatorOpt.utf8_lead_bit_sound",
        "ComparatorOpt.horizontal_byte_sum_sound",
        "ComparatorOpt.mersenne31_fast_sound",
    ];
    let ok = verify_lean_axiom_log(&real_log, required).unwrap();
    assert_eq!(ok.len(), 5);

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

    // 3. Reject missing theorem audit entry.
    assert_eq!(
        verify_lean_axiom_log(&real_log, &["ComparatorOpt.missing_theorem"]),
        Err(LockError::MissingAxiomAudit {
            theorem: "ComparatorOpt.missing_theorem".to_owned(),
        })
    );
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

    // Demonstrate why `ref_mersenne31_valid` (`x < (2^31 - 1)^2`) is non-trivial:
    // at `u64::MAX` (outside ValidInput), two 31-bit folds leave `u >= 2^31`.
    assert!(!ref_mersenne31_valid(u64::MAX));
    assert_ne!(fast_mersenne31_mod(u64::MAX), ref_mersenne31_mod(u64::MAX));
}
