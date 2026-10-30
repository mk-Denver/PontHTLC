use lightning::types::payment::{PaymentHash, PaymentPreimage};
use ponthtlc_protocol::event::NostrEvent;
use ponthtlc_protocol::swap_v1::{reconstruct, SwapRoot, SwapState, SwapStatus};

use crate::ldk_bridge::LdkBridge;

pub struct EscrowMachine<N: crate::ldk_bridge::EscrowNode> {
    root: SwapRoot,
    payment_hash: PaymentHash,
    amount_msat: u64,
    bridge: LdkBridge<N>,
    last_status: SwapStatus,
    chain_len: usize,
}

impl<N: crate::ldk_bridge::EscrowNode> EscrowMachine<N> {
    pub fn new(
        root: SwapRoot,
        payment_hash: PaymentHash,
        amount_msat: u64,
        bridge: LdkBridge<N>,
    ) -> Self {
        Self {
            root,
            payment_hash,
            amount_msat,
            bridge,
            last_status: SwapStatus::Proposed,
            chain_len: 0,
        }
    }

    pub fn root(&self) -> &SwapRoot {
        &self.root
    }

    pub fn status(&self) -> &SwapStatus {
        &self.last_status
    }

    pub fn replay(
        &mut self,
        events: &[NostrEvent],
        verify: bool,
    ) -> Result<SwapState, String> {
        let state = reconstruct(&self.root, events, verify);
        while self.chain_len < state.chain.len() {
            let applied = &state.chain[self.chain_len];
            let next_status = derive_status_after(&self.last_status, &applied.action);
            if let Some(next) = next_status {
                if next == SwapStatus::Settled {
                    return Err("use settle_with_preimage to settle".to_string());
                }
                self.bridge.apply_state_transition(
                    &self.last_status,
                    &next,
                    &self.payment_hash,
                    self.amount_msat,
                    None,
                )?;
                self.last_status = next;
            }
            self.chain_len += 1;
        }
        if self.last_status != state.status {
            self.last_status = state.status.clone();
        }
        Ok(state)
    }

    pub fn settle_with_preimage(&mut self, preimage: PaymentPreimage) -> Result<(), String> {
        if self.last_status != SwapStatus::SettlementAuthorized {
            return Err("settlement not authorized".to_string());
        }
        self.bridge.settle(preimage)?;
        self.last_status = SwapStatus::Settled;
        Ok(())
    }

    pub fn payment_hash(&self) -> &PaymentHash {
        &self.payment_hash
    }

    pub fn amount_msat(&self) -> u64 {
        self.amount_msat
    }
}

fn derive_status_after(prev: &SwapStatus, action: &str) -> Option<SwapStatus> {
    match (prev, action) {
        (SwapStatus::Proposed, "core/accept") => Some(SwapStatus::Accepted),
        (SwapStatus::Proposed, "core/decline") => Some(SwapStatus::Declined),
        (SwapStatus::Proposed, "core/cancel") => Some(SwapStatus::Cancelled),
        (SwapStatus::Proposed, "core/expire") => Some(SwapStatus::Expired),
        (SwapStatus::Accepted, "core/secure") => Some(SwapStatus::Secured),
        (SwapStatus::Secured, "swap/fiat_sent") => Some(SwapStatus::FiatSent),
        (SwapStatus::FiatSent, "swap/fiat_confirmed") => Some(SwapStatus::FiatConfirmed),
        (SwapStatus::FiatConfirmed, "core/authorize_settlement") => Some(SwapStatus::SettlementAuthorized),
        (SwapStatus::SettlementAuthorized, "core/settle") => Some(SwapStatus::Settled),
        (SwapStatus::Secured, "core/authorize_refund") => Some(SwapStatus::RefundAuthorized),
        (SwapStatus::RefundAuthorized, "core/refund") => Some(SwapStatus::Refunded),
        _ => None,
    }
}