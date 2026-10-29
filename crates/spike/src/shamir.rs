use bitcoin::hashes::Hash;
use lightning::types::payment::{PaymentHash, PaymentPreimage};
use rand::Rng;

const GF_POLY: u8 = 0x1b;

pub const SHARE_X_BUYER: u8 = 1;
pub const SHARE_X_SELLER: u8 = 2;
pub const SHARE_X_ARBITRATOR: u8 = 3;

#[derive(Clone, Copy)]
pub struct Share {
    pub x: u8,
    pub bytes: [u8; 32],
}

impl Share {
    pub fn combine_with(self, other: &Share) -> [u8; 32] {
        let denom = self.x ^ other.x;
        let denom_inv = gf_inv(denom);
        let mut secret = [0u8; 32];
        for i in 0..32 {
            let term_a = gf_mul(self.bytes[i], gf_mul(other.x, denom_inv));
            let term_b = gf_mul(other.bytes[i], gf_mul(self.x, denom_inv));
            secret[i] = term_a ^ term_b;
        }
        secret
    }
}

fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut product: u8 = 0;
    while b != 0 {
        if b & 1 != 0 {
            product ^= a;
        }
        let high_bit = a & 0x80;
        a = a << 1;
        if high_bit != 0 {
            a ^= GF_POLY;
        }
        b >>= 1;
    }
    product
}

fn gf_pow(mut base: u8, mut exponent: u8) -> u8 {
    let mut accumulator: u8 = 1;
    while exponent > 0 {
        if exponent & 1 != 0 {
            accumulator = gf_mul(accumulator, base);
        }
        base = gf_mul(base, base);
        exponent >>= 1;
    }
    accumulator
}

fn gf_inv(value: u8) -> u8 {
    gf_pow(value, 254)
}

fn eval_poly(coeffs: &[u8; 2], x: u8) -> u8 {
    let mut value: u8 = coeffs[0];
    let mut x_power: u8 = 1;
    for coeff in &coeffs[1..] {
        x_power = gf_mul(x_power, x);
        value ^= gf_mul(*coeff, x_power);
    }
    value
}

pub fn split_preimage(preimage: &[u8; 32]) -> [Share; 3] {
    let share_xs = [SHARE_X_BUYER, SHARE_X_SELLER, SHARE_X_ARBITRATOR];
    let mut rng = rand::thread_rng();
    let mut shares = [Share { x: share_xs[0], bytes: [0u8; 32] }, Share { x: share_xs[1], bytes: [0u8; 32] }, Share { x: share_xs[2], bytes: [0u8; 32] }];
    for i in 0..32 {
        let mut coeffs = [0u8; 2];
        coeffs[0] = preimage[i];
        rng.fill(&mut coeffs[1..]);
        for (share, share_x) in shares.iter_mut().zip(share_xs.iter()) {
            share.bytes[i] = eval_poly(&coeffs, *share_x);
        }
    }
    shares
}

pub fn reconstruct_and_verify(
    share_a: &Share, share_b: &Share, expected_hash: &PaymentHash,
) -> Option<PaymentPreimage> {
    let preimage = share_a.combine_with(share_b);
    let effective_hash = bitcoin::hashes::sha256::Hash::hash(&preimage);
    if effective_hash.to_byte_array() == expected_hash.0 {
        Some(PaymentPreimage(preimage))
    } else {
        None
    }
}
