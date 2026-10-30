use std::sync::Mutex;

use bitcoin::hashes::sha256::Hash as Sha256;
use bitcoin::hashes::Hash;
use lightning::types::payment::{PaymentHash, PaymentPreimage, PaymentSecret};
use ponthtlc_escrow::shamir::*;
use ponthtlc_escrow::ldk_bridge::{EscrowNode, LdkBridge};
use ponthtlc_escrow::machine::EscrowMachine;
use ponthtlc_protocol::swap_v1::SwapStatus;

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
    assert_eq!(s1.combine_with(s2), preimage);
    assert_eq!(s1.combine_with(s3), preimage);
    assert_eq!(s2.combine_with(s3), preimage);
}

#[test]
fn split_then_verify_hash() {
    let preimage = [0xABu8; 32];
    let hash = PaymentHash(Sha256::hash(&preimage).to_byte_array());
    let shares = split_preimage(&preimage);
    let result = reconstruct_and_verify(&shares[0], &shares[1], &hash);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), PaymentPreimage(preimage));
}

#[test]
fn split_random_all_pairs() {
    let mut preimage = [0u8; 32];
    rand::thread_rng().fill(&mut preimage);
    let shares = split_preimage(&preimage);
    assert_eq!(shares[0].combine_with(&shares[1]), preimage);
    assert_eq!(shares[0].combine_with(&shares[2]), preimage);
    assert_eq!(shares[1].combine_with(&shares[2]), preimage);
}

#[test]
fn reconstruct_wrong_pair_fails() {
    let preimage_a = [0xAAu8; 32];
    let preimage_b = [0xBBu8; 32];
    let hash_a = PaymentHash(Sha256::hash(&preimage_a).to_byte_array());
    let shares_a = split_preimage(&preimage_a);
    let shares_b = split_preimage(&preimage_b);
    let result = reconstruct_and_verify(&shares_a[0], &shares_b[1], &hash_a);
    assert!(result.is_none());
}

struct MockEscrowNode {
    registered: Mutex<Vec<PaymentHash>>,
    claimed: Mutex<Vec<PaymentPreimage>>,
    failed: Mutex<Vec<PaymentHash>>,
}

impl MockEscrowNode {
    fn new() -> Self {
        Self {
            registered: Mutex::new(Vec::new()),
            claimed: Mutex::new(Vec::new()),
            failed: Mutex::new(Vec::new()),
        }
    }
}

impl EscrowNode for MockEscrowNode {
    fn register_hold(
        &self,
        payment_hash: PaymentHash,
        _amount_msat: u64,
    ) -> Result<PaymentSecret, String> {
        self.registered.lock().unwrap().push(payment_hash);
        Ok(PaymentSecret([0u8; 32]))
    }

    fn claim(&self, preimage: PaymentPreimage) -> Result<(), String> {
        self.claimed.lock().unwrap().push(preimage);
        Ok(())
    }

    fn fail_htlc(&self, payment_hash: PaymentHash) -> Result<(), String> {
        self.failed.lock().unwrap().push(payment_hash);
        Ok(())
    }
}

#[test]
fn bridge_secure_returns_payment_secret() {
    let node = MockEscrowNode::new();
    let bridge = LdkBridge::new(node);
    let hash = PaymentHash([1u8; 32]);
    let secret = bridge.secure(&hash, 50_000_000).unwrap();
    assert_eq!(secret.0, [0u8; 32]);
}

#[test]
fn bridge_settle_rejects_wrong_preimage() {
    let node = MockEscrowNode::new();
    let bridge = LdkBridge::new(node);
    let preimage = PaymentPreimage([2u8; 32]);
    let wrong_hash = PaymentHash([9u8; 32]);
    let result = bridge.settle(preimage, &wrong_hash);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("preimage does not match"));
}

#[test]
fn bridge_settle_accepts_matching_preimage() {
    let node = MockEscrowNode::new();
    let bridge = LdkBridge::new(node);
    let preimage = PaymentPreimage([3u8; 32]);
    let hash = PaymentHash(Sha256::hash(&preimage.0).to_byte_array());
    bridge.settle(preimage, &hash).unwrap();
}

use rand::Rng;
use rand::RngCore;