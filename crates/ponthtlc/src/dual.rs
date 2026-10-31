use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use lightning::events::Event;
use tokio::sync::broadcast;

use crate::bitcoind::BitcoindFacade;
use crate::events::{event_channel, WireEvent};
use crate::node::{build_node, ChannelSnapshot, Node};

pub const MIN_CHANNEL_SATOSHIS: u64 = 1_000_000;

#[derive(Clone)]
pub struct DualNode {
    pub buyer: Arc<Node>,
    pub seller: Arc<Node>,
}

impl DualNode {
    pub async fn start(
        data_dir: PathBuf, buyer_port: u16, seller_port: u16, bitcoind: Arc<BitcoindFacade>,
    ) -> Result<Self, String> {
        let buyer_dir = data_dir.join("buyer");
        let seller_dir = data_dir.join("seller");
        let buyer = build_node("buyer", buyer_dir, buyer_port, Arc::clone(&bitcoind)).await;
        let seller = build_node("seller", seller_dir, seller_port, bitcoind).await;
        let pair = DualNode { buyer, seller };
        pair.connect_internal().await?;
        Ok(pair)
    }

    pub fn peers_connected(&self) -> bool {
        self.seller.is_connected_to(&self.buyer.node_pubkey)
            && self.buyer.is_connected_to(&self.seller.node_pubkey)
    }

    pub async fn connect_internal(&self) -> Result<(), String> {
        if self.peers_connected() {
            return Ok(());
        }
        self.seller
            .remember_peer(self.buyer.node_pubkey, self.buyer.listen_addr)
            .await?;
        for _ in 0..100 {
            if self.peers_connected() {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Err("buyer and seller nodes did not complete the peer handshake".to_string())
    }

    pub fn existing_channel_id(&self) -> Option<String> {
        self.seller
            .channel_manager
            .list_channels()
            .into_iter()
            .find(|channel| channel.counterparty.node_id == self.buyer.node_pubkey)
            .map(|channel| channel.channel_id.to_string())
    }

    pub fn has_usable_route(&self) -> bool {
        self.seller.channel_manager.list_usable_channels().iter().any(|channel| {
            channel.counterparty.node_id == self.buyer.node_pubkey
        })
    }

    pub async fn open_internal_channel(&self, capacity_satoshis: u64) -> Result<String, String> {
        if capacity_satoshis < MIN_CHANNEL_SATOSHIS {
            return Err(format!(
                "capacity_satoshis must be at least {MIN_CHANNEL_SATOSHIS}"
            ));
        }
        self.connect_internal().await?;
        if let Some(channel_id) = self.existing_channel_id() {
            return Ok(channel_id);
        }
        let channel_id = self
            .seller
            .channel_manager
            .create_channel(self.buyer.node_pubkey, capacity_satoshis, 0, 0, None, None)
            .map_err(|err| format!("create_channel failed: {:?}", err))?;
        self.seller
            .wait_for_event(|event| matches!(event, Event::FundingGenerationReady { .. }), 30)
            .await
            .ok_or_else(|| "FundingGenerationReady never fired".to_string())?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        Ok(channel_id.to_string())
    }

    pub fn channel_ids(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut ids = Vec::new();
        for id in self
            .buyer
            .listed_channel_ids()
            .into_iter()
            .chain(self.seller.listed_channel_ids())
        {
            if seen.insert(id.clone()) {
                ids.push(id);
            }
        }
        ids
    }

    pub fn channel_snapshots(&self) -> Vec<ChannelSnapshot> {
        let mut seen = HashSet::new();
        let mut snapshots = Vec::new();
        for snapshot in self
            .seller
            .channel_snapshots()
            .into_iter()
            .chain(self.buyer.channel_snapshots())
        {
            if seen.insert(snapshot.channel_id.clone()) {
                snapshots.push(snapshot);
            }
        }
        snapshots
    }

    pub fn subscribe_merged(&self) -> broadcast::Receiver<WireEvent> {
        let (tx, rx) = event_channel();
        let mut buyer_rx = self.buyer.subscribe();
        let mut seller_rx = self.seller.subscribe();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    event = buyer_rx.recv() => {
                        match event {
                            Ok(event) => {
                                let _ = tx.send(event);
                            },
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                    event = seller_rx.recv() => {
                        match event {
                            Ok(event) => {
                                let _ = tx.send(event);
                            },
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                }
            }
        });
        rx
    }

    pub async fn shutdown(&self) {
        self.seller.shutdown().await;
        self.buyer.shutdown().await;
    }
}
