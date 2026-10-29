use bitcoin::hashes::Hash;

use ponthtlc_spike::shamir::{
    eval_poly, gf_inv, gf_mul, gf_pow, reconstruct_and_verify, split_preimage, Share,
    SHARE_X_ARBITRATOR, SHARE_X_BUYER, SHARE_X_SELLER,
};
use lightning::types::payment::{PaymentHash, PaymentPreimage};
use rand::Rng;

#[test]
fn gf_mul_identity() {
    for a in 1..=255u8 {
        assert_eq!(gf_mul(a, 1), a);
    }
}

#[test]
fn gf_mul_zero() {
    for a in 0..=255u8 {
        assert_eq!(gf_mul(a, 0), 0);
    }
}

#[test]
fn gf_mul_commutative() {
    for a in 0..=255u8 {
        for b in 0..255u8 {
            assert_eq!(gf_mul(a, b), gf_mul(b, a));
        }
    }
}

#[test]
fn gf_mul_associative() {
    for a in 1..=255u8 {
        for b in 1..=255u8 {
            for c in 1..=255u8 {
                assert_eq!(gf_mul(gf_mul(a, b), c), gf_mul(a, gf_mul(b, c)));
            }
        }
    }
}

#[test]
fn gf_inv_identity() {
    for a in 1..=255u8 {
        let inv = gf_inv(a);
        assert_eq!(gf_mul(a, inv), 1, "a={} inv={} product={}", a, inv, gf_mul(a, inv));
    }
}

#[test]
fn gf_inv_zero_returns_zero() {
    assert_eq!(gf_inv(0), 0);
}

#[test]
fn eval_poly_at_zero() {
    let coeffs = [42u8, 99u8];
    assert_eq!(eval_poly(&coeffs, 0), 42);
}

#[test]
fn eval_poly_at_one() {
    let coeffs = [10u8, 20u8];
    assert_eq!(eval_poly(&coeffs, 1), 10 ^ 20);
}

#[test]
fn split_then_combine_all_pairs() {
    let preimage = [0x42u8; 32];
    let shares = split_preimage(&preimage);
    let s1 = &shares[0];
    let s2 = &shares[1];
    let s3 = &shares[2];

    assert_eq!(s1.x, SHARE_X_BUYER);
    assert_eq!(s2.x, SHARE_X_SELLER);
    assert_eq!(s3.x, SHARE_X_ARBITRATOR);

    let reconstructed_12 = s1.combine_with(s2);
    let reconstructed_13 = s1.combine_with(s3);
    let reconstructed_23 = s2.combine_with(s3);

    assert_eq!(reconstructed_12, preimage);
    assert_eq!(reconstructed_13, preimage);
    assert_eq!(reconstructed_23, preimage);
}

#[test]
fn split_then_verify_hash() {
    let preimage = [0xABu8; 32];
    let hash = PaymentHash(bitcoin::hashes::sha256::Hash::hash(&preimage).to_byte_array());
    let shares = split_preimage(&preimage);

    let result = reconstruct_and_verify(&shares[0], &shares[1], &hash);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), PaymentPreimage(preimage));
}

#[test]
fn split_random_preimage_all_pairs_reconstruct() {
    let mut preimage = [0u8; 32];
    rand::thread_rng().fill(&mut preimage);

    let shares = split_preimage(&preimage);
    let reconstructed_12 = shares[0].combine_with(&shares[1]);
    let reconstructed_13 = shares[0].combine_with(&shares[2]);
    let reconstructed_23 = shares[1].combine_with(&shares[2]);

    assert_eq!(reconstructed_12, preimage);
    assert_eq!(reconstructed_13, preimage);
    assert_eq!(reconstructed_23, preimage);
}

#[test]
fn reconstruct_wrong_pair_returns_none_on_hash_mismatch() {
    let preimage_a = [0xAAu8; 32];
    let preimage_b = [0xBBu8; 32];
    let hash_a = PaymentHash(bitcoin::hashes::sha256::Hash::hash(&preimage_a).to_byte_array());

    let shares_a = split_preimage(&preimage_a);
    let shares_b = split_preimage(&preimage_b);

    let result = reconstruct_and_verify(&shares_a[0], &shares_b[1], &hash_a);
    assert!(result.is_none());
}

#[test]
fn gf_pow_zero() {
    assert_eq!(gf_pow(5, 0), 1);
}

#[test]
fn gf_pow_one() {
    assert_eq!(gf_pow(5, 1), 5);
}

#[test]
fn gf_pow_two() {
    assert_eq!(gf_pow(5, 2), gf_mul(5, 5));
}
