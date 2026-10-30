use bitcoin::hashes::sha256::Hash as Sha256;
use bitcoin::hashes::Hash;
use lightning::types::payment::{PaymentHash, PaymentPreimage, PaymentSecret};
use ponthtlc_protocol::swap_v1::SwapStatus;

pub trait EscrowNode: Send + Sync {
    fn register_hold(
        &self,
        payment_hash: PaymentHash,
        amount_msat: u64,
    ) -> Result<PaymentSecret, String>;
    fn claim(&self, preimage: PaymentPreimage) -> Result<(), String>;
    fn fail_htlc(&self, payment_hash: PaymentHash) -> Result<(), String>;
}

pub struct LdkBridge<N: EscrowNode> {
    node: N,
}

impl<N: EscrowNode> LdkBridge<N> {
    pub fn new(node: N) -> Self {
        Self { node }
    }

    pub fn secure(
        &self,
        payment_hash: &PaymentHash,
        amount_msat: u64,
    ) -> Result<PaymentSecret, String> {
        self.node.register_hold(*payment_hash, amount_msat)
    }

    pub fn settle(
        &self,
        preimage: PaymentPreimage,
        payment_hash: &PaymentHash,
    ) -> Result<(), String> {
        let computed = Sha256::hash(&preimage.0).to_byte_array();
        if computed != payment_hash.0 {
            return Err("preimage does not match payment hash".to_string());
        }
        self.node.claim(preimage)
    }

    pub fn refund(&self, payment_hash: &PaymentHash) -> Result<(), String> {
        self.node.fail_htlc(*payment_hash)
    }

    pub fn apply_state_transition(
        &self,
        prev: &SwapStatus,
        next: &SwapStatus,
        payment_hash: &PaymentHash,
        amount_msat: u64,
        preimage: Option<&PaymentPreimage>,
    ) -> Result<(), String> {
        match (prev, next) {
            (SwapStatus::Accepted, SwapStatus::Secured) => {
                self.secure(payment_hash, amount_msat)?;
                Ok(())
            }
            (SwapStatus::SettlementAuthorized, SwapStatus::Settled) => {
                let p = preimage.ok_or("preimage required for settlement")?;
                self.settle(*p, payment_hash)
            }
            (SwapStatus::RefundAuthorized, SwapStatus::Refunded) => {
                self.refund(payment_hash)
            }
            _ => Ok(()),
        }
    }

    pub fn node(&self) -> &N {
        &self.node
    }
}