use std::sync::Arc;

use lightning::types::payment::{PaymentHash, PaymentPreimage};
use ponthtlc_protocol::swap_v1::SwapStatus;

use ponthtlc::node::Node;

pub trait EscrowNode: Send + Sync {
    fn register_hold(&self, payment_hash: PaymentHash, amount_msat: u64) -> Result<(), String>;
    fn claim(&self, preimage: PaymentPreimage) -> Result<(), String>;
    fn fail_htlc(&self, payment_hash: PaymentHash) -> Result<(), String>;
}

impl EscrowNode for Arc<Node> {
    fn register_hold(&self, payment_hash: PaymentHash, amount_msat: u64) -> Result<(), String> {
        self.channel_manager
            .create_inbound_payment_for_hash(payment_hash, Some(amount_msat), 3600, None)
            .map(|_| ())
            .map_err(|e| format!("create_inbound_payment_for_hash failed: {:?}", e))
    }

    fn claim(&self, preimage: PaymentPreimage) -> Result<(), String> {
        self.channel_manager.claim_funds(preimage);
        Ok(())
    }

    fn fail_htlc(&self, payment_hash: PaymentHash) -> Result<(), String> {
        self.channel_manager.fail_htlc_backwards(&payment_hash);
        Ok(())
    }
}

pub struct LdkBridge<N: EscrowNode> {
    node: N,
}

impl<N: EscrowNode> LdkBridge<N> {
    pub fn new(node: N) -> Self {
        Self { node }
    }

    pub fn secure(&self, payment_hash: &PaymentHash, amount_msat: u64) -> Result<(), String> {
        self.node.register_hold(*payment_hash, amount_msat)
    }

    pub fn settle(&self, preimage: PaymentPreimage) -> Result<(), String> {
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
                self.secure(payment_hash, amount_msat)
            }
            (SwapStatus::SettlementAuthorized, SwapStatus::Settled) => {
                let p = preimage.ok_or("preimage required for settlement")?;
                self.settle(*p)
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