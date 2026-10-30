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
        let new_status = state.status.clone();
        if new_status != self.last_status {
            self.bridge.apply_state_transition(
                &self.last_status,
                &new_status,
                &self.payment_hash,
                self.amount_msat,
                None,
            )?;
            self.last_status = new_status;
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