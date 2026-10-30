use std::sync::{Arc, Mutex};

use lightning::events::{Event, EventHandler, ReplayEvent};
use lightning::types::payment::PaymentHash;
use serde::Serialize;
use tokio::sync::broadcast;

use crate::bitcoind::BitcoindFacade;
use crate::node::{ChannelManagerT, EventSink};

#[derive(Clone, Debug, Serialize)]
pub struct WireEvent {
    pub kind: String,
    pub payment_hash: Option<String>,
}

pub fn redact(event: &Event) -> WireEvent {
    let (kind, payment_hash) = match event {
        Event::FundingGenerationReady { .. } => ("FundingGenerationReady", None),
        Event::PaymentClaimable { payment_hash, .. } => {
            ("PaymentClaimable", Some(hash_hex(payment_hash)))
        },
        Event::PaymentClaimed { payment_hash, .. } => ("PaymentClaimed", Some(hash_hex(payment_hash))),
        Event::PaymentSent { payment_hash, .. } => ("PaymentSent", Some(hash_hex(payment_hash))),
        Event::PaymentFailed { payment_hash, .. } => {
            ("PaymentFailed", payment_hash.as_ref().map(hash_hex))
        },
        Event::ChannelReady { .. } => ("ChannelReady", None),
        Event::ChannelPending { .. } => ("ChannelPending", None),
        _ => ("Event", None),
    };
    WireEvent { kind: kind.to_string(), payment_hash }
}

fn hash_hex(payment_hash: &PaymentHash) -> String {
    hex::encode(payment_hash.0)
}

pub struct NodeEventHandler {
    pub name: String,
    pub channel_manager: Arc<ChannelManagerT>,
    pub bitcoind: Arc<BitcoindFacade>,
    pub events: EventSink,
    pub broadcast: broadcast::Sender<WireEvent>,
}

impl EventHandler for NodeEventHandler {
    fn handle_event(&self, event: Event) -> Result<(), ReplayEvent> {
        let wire = redact(&event);
        match &wire.payment_hash {
            Some(hash) => println!("[{}] event={} hash={}", self.name, wire.kind, hash),
            None => println!("[{}] event={}", self.name, wire.kind),
        }
        if let Event::FundingGenerationReady {
            temporary_channel_id,
            counterparty_node_id,
            channel_value_satoshis,
            output_script,
            ..
        } = &event
        {
            // BackgroundProcessor runs on its own thread, outside the tokio worker, so the
            // bitcoind RPC future has to be driven with the runtime handle captured at startup.
            let funded = self.bitcoind.block_on(
                self.bitcoind.create_funding_transaction(output_script, *channel_value_satoshis),
            );
            match funded {
                Ok(funding_tx) => {
                    if let Err(e) = self.channel_manager.funding_transaction_generated(
                        *temporary_channel_id,
                        *counterparty_node_id,
                        funding_tx,
                    ) {
                        println!(
                            "[{}] SPIKE_NOTE: funding_transaction_generated rejected: {:?}",
                            self.name, e
                        );
                    }
                },
                Err(e) => {
                    println!(
                        "[{}] SPIKE_NOTE: failed to create funding transaction: {}",
                        self.name, e
                    );
                    return Err(ReplayEvent());
                },
            }
        }
        let _ = self.broadcast.send(wire);
        self.events.lock().expect("event sink mutex poisoned").push(event);
        Ok(())
    }
}

pub type EventBroadcast = broadcast::Sender<WireEvent>;

pub fn event_channel() -> (EventBroadcast, broadcast::Receiver<WireEvent>) {
    broadcast::channel(256)
}

pub fn sink() -> EventSink {
    Arc::new(Mutex::new(Vec::new()))
}
