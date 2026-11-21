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
fn sha256_matches_python_hashlib_at_padding_boundaries() {
    // Lengths around the one-block / two-block padding split (55, 56) and block edges (63, 64,
    // 119, 120, 128). Input byte i is `i % 251`. Expected digests come from Python `hashlib`.
    let cases = "\
55 463eb28e72f82e0a96c0a4cc53690c571281131f672aa229e0d45ae59b598b59
56 da2ae4d6b36748f2a318f23e7ab1dfdf45acdc9d049bd80e59de82a60895f562
57 2fe741af801cc238602ac0ec6a7b0c3a8a87c7fc7d7f02a3fe03d1c12eac4d8f
63 29af2686fd53374a36b0846694cc342177e428d1647515f078784d69cdb9e488
64 fdeab9acf3710362bd2658cdc9a29e8f9c757fcf9811603a8c447cd1d9151108
65 4bfd2c8b6f1eec7a2afeb48b934ee4b2694182027e6d0fc075074f2fabb31781
119 da18797ed7c3a777f0847f429724a2d8cd5138e6ed2895c3fa1a6d39d18f7ec6
120 f52b23db1fbb6ded89ef42a23ce0c8922c45f25c50b568a93bf1c075420bbb7c
128 471fb943aa23c511f6f72f8d1652d9c880cfa392ad80503120547703e56a2be5
";
    for case in cases.lines() {
        let (len, expected) = case.split_once(' ').unwrap();
        let len: usize = len.parse().unwrap();
        let data: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
        assert_eq!(sha256_hex(&data), expected, "length {len}");
    }
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
