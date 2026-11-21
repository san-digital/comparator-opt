use comparator_opt::{
    sha256_hex, swar_ascii_lower_u64, verify_kernel, AsciiLowerKernel, Mersenne31BatchKernel,
    Utf8CountKernel, VerifyConfig,
};

#[test]
fn sha256_matches_nist_fips_180_4_vectors() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
    let million_a = vec![b'a'; 1_000_000];
    assert_eq!(
        sha256_hex(&million_a),
        "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
    );
}

#[test]
fn all_three_superoptimized_kernels_pass_full_differential_harness() {
    let cfg = VerifyConfig {
        samples: 5_000,
        bench_iters: 50,
        ..VerifyConfig::default()
    };

    let r1 = verify_kernel(&AsciiLowerKernel, &cfg).unwrap();
    assert_eq!(r1.valid_bps, 10_000);

    let r2 = verify_kernel(&Utf8CountKernel, &cfg).unwrap();
    assert_eq!(r2.valid_bps, 10_000);

    let r3 = verify_kernel(&Mersenne31BatchKernel, &cfg).unwrap();
    assert!(r3.valid_bps >= 8_000);

    // Exhaustive single-byte check across all 256 byte values in every lane position 0..8.
    for lane in 0..8 {
        for b in 0u8..=255 {
            let word = (b as u64) << (8 * lane);
            let out = swar_ascii_lower_u64(word);
            let got = ((out >> (8 * lane)) & 0xff) as u8;
            let expected = comparator_opt::ref_ascii_lower_byte(b);
            assert_eq!(got, expected, "mismatch at lane={lane}, byte={b:#04x}");
        }
    }
}
