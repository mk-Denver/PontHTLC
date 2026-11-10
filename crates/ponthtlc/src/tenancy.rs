use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use ponthtlc_protocol::event::NostrEvent;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EscrowRecord {
    pub id: String,
    pub owner: String,
    pub payment_hash: String,
    pub state: String,
    #[serde(default)]
    pub bolt11: String,
    #[serde(default)]
    pub protocol_status: String,
    #[serde(default)]
    pub root_id: Option<String>,
    #[serde(default)]
    pub tip_id: Option<String>,
    #[serde(default)]
    pub amount_msat: u64,
    #[serde(default)]
    pub buyer_lightning_address: String,
    #[serde(default)]
    pub seller_lightning_address: String,
    #[serde(default)]
    pub seller_owner: String,
    #[serde(default)]
    pub payout: String,
}

impl Default for EscrowRecord {
    fn default() -> Self {
        Self {
            id: String::new(),
            owner: String::new(),
            payment_hash: String::new(),
            state: String::new(),
            bolt11: String::new(),
            protocol_status: String::new(),
            root_id: None,
            tip_id: None,
            amount_msat: 0,
            buyer_lightning_address: String::new(),
            seller_lightning_address: String::new(),
            seller_owner: String::new(),
            payout: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ChannelAssignment {
    owner: String,
    channel_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct OutgoingPayment {
    payer: String,
    payment_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct BalanceEntry {
    owner: String,
    msat: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct DiskState {
    escrows: Vec<EscrowRecord>,
    channels: Vec<ChannelAssignment>,
    #[serde(default)]
    outgoing: Vec<OutgoingPayment>,
    #[serde(default)]
    event_chains: std::collections::HashMap<String, Vec<NostrEvent>>,
    #[serde(default)]
    balances: Vec<BalanceEntry>,
}

pub struct EscrowStore {
    path: PathBuf,
    state: DiskState,
}

impl EscrowStore {
    pub fn open(path: PathBuf) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let state = if path.exists() {
            let bytes = fs::read(&path).map_err(|e| e.to_string())?;
            serde_json::from_slice(&bytes).map_err(|e| e.to_string())?
        } else {
            DiskState::default()
        };
        let store = EscrowStore { path, state };
        store.persist()?;
        Ok(store)
    }

    pub fn insert(&mut self, record: EscrowRecord) -> Result<(), String> {
        if self.state.escrows.iter().any(|existing| existing.id == record.id) {
            return Err("escrow id already exists".to_string());
        }
        self.state.escrows.push(record);
        self.persist()
    }

    pub fn list(&self, owner: &str) -> Vec<EscrowRecord> {
        self.state.escrows.iter().filter(|record| record.owner == owner).cloned().collect()
    }

    pub fn list_visible(&self, owner: &str) -> Vec<EscrowRecord> {
        self.state
            .escrows
            .iter()
            .filter(|record| {
                record.owner == owner
                    || record.seller_owner == owner
                    || record.protocol_status == "proposed"
                    || record.protocol_status == "secured"
            })
            .cloned()
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<EscrowRecord> {
        self.state.escrows.iter().find(|record| record.id == id).cloned()
    }

    pub fn get_owned(&self, id: &str, owner: &str) -> Option<EscrowRecord> {
        self.state
            .escrows
            .iter()
            .find(|record| record.id == id && record.owner == owner)
            .cloned()
    }

    pub fn visible_hash(&self, owner: &str, payment_hash: &str) -> bool {
        self.state.escrows.iter().any(|record| {
            record.owner == owner && record.payment_hash == payment_hash
        }) || self.state.outgoing.iter().any(|payment| {
            payment.payer == owner && payment.payment_hash == payment_hash
        })
    }

    pub fn record_outgoing(&mut self, payer: &str, payment_hash: &str) -> Result<(), String> {
        if self.state.outgoing.iter().any(|payment| {
            payment.payer == payer && payment.payment_hash == payment_hash
        }) {
            return Ok(());
        }
        self.state.outgoing.push(OutgoingPayment {
            payer: payer.to_string(),
            payment_hash: payment_hash.to_string(),
        });
        self.persist()
    }

    pub fn assign_channel(&mut self, owner: &str, channel_id: &str) -> Result<(), String> {
        if self.state.channels.iter().any(|channel| channel.channel_id == channel_id) {
            return Err("channel already assigned".to_string());
        }
        self.state.channels.push(ChannelAssignment {
            owner: owner.to_string(),
            channel_id: channel_id.to_string(),
        });
        self.persist()
    }

    pub fn channels_for(&self, owner: &str) -> Vec<String> {
        self.state
            .channels
            .iter()
            .filter(|channel| channel.owner == owner)
            .map(|channel| channel.channel_id.clone())
            .collect()
    }

    pub fn mark_claim_submitted(&mut self, id: &str, owner: &str) -> Result<(), String> {
        let record = self
            .state
            .escrows
            .iter_mut()
            .find(|record| record.id == id && record.owner == owner)
            .ok_or_else(|| "escrow not found".to_string())?;
        record.state = "claim_submitted".to_string();
        record.protocol_status = "settled".to_string();
        self.persist()
    }

    pub fn mark_fail_submitted(&mut self, id: &str, owner: &str) -> Result<(), String> {
        let record = self
            .state
            .escrows
            .iter_mut()
            .find(|record| record.id == id && record.owner == owner)
            .ok_or_else(|| "escrow not found".to_string())?;
        record.state = "fail_submitted".to_string();
        record.protocol_status = "refunded".to_string();
        self.persist()
    }

    pub fn mark_disputed(&mut self, id: &str, owner: &str) -> Result<(), String> {
        let record = self
            .state
            .escrows
            .iter_mut()
            .find(|record| record.id == id && record.owner == owner)
            .ok_or_else(|| "escrow not found".to_string())?;
        record.protocol_status = "disputed".to_string();
        self.persist()
    }

    pub fn mark_protocol_status(&mut self, id: &str, status: &str) -> Result<(), String> {
        let record = self
            .state
            .escrows
            .iter_mut()
            .find(|record| record.id == id)
            .ok_or_else(|| "escrow not found".to_string())?;
        record.protocol_status = status.to_string();
        self.persist()
    }

    pub fn by_payment_hash(&self, payment_hash: &str) -> Option<EscrowRecord> {
        self.state
            .escrows
            .iter()
            .find(|record| record.payment_hash == payment_hash)
            .cloned()
    }

    pub fn balance_of(&self, owner: &str) -> u64 {
        self.state
            .balances
            .iter()
            .find(|row| row.owner == owner)
            .map(|row| row.msat)
            .unwrap_or(0)
    }

    pub fn owed(&self) -> Vec<EscrowRecord> {
        self.state
            .escrows
            .iter()
            .filter(|record| record.payout == "buyer" || record.payout == "seller")
            .cloned()
            .collect()
    }

    pub fn set_seller_destination(
        &mut self, id: &str, seller_owner: &str, address: &str,
    ) -> Result<(), String> {
        let record = self
            .state
            .escrows
            .iter_mut()
            .find(|record| record.id == id)
            .ok_or_else(|| "escrow not found".to_string())?;
        record.seller_owner = seller_owner.to_string();
        if !address.is_empty() {
            record.seller_lightning_address = address.to_string();
        }
        self.persist()
    }

    pub fn credit_payout(&mut self, id: &str, side: &str) -> Result<bool, String> {
        let (owner, amount) = {
            let record = self
                .state
                .escrows
                .iter_mut()
                .find(|record| record.id == id)
                .ok_or_else(|| "escrow not found".to_string())?;
            if record.payout == "paid" || record.payout == side {
                return Ok(false);
            }
            if !record.payout.is_empty() {
                return Err("payout already assigned".to_string());
            }
            let owner = if side == "buyer" {
                record.owner.clone()
            } else if side == "seller" {
                record.seller_owner.clone()
            } else {
                return Err("unknown payout side".to_string());
            };
            if owner.is_empty() || record.amount_msat == 0 {
                return Ok(false);
            }
            let amount = record.amount_msat;
            record.payout = side.to_string();
            (owner, amount)
        };
        if let Some(row) = self.state.balances.iter_mut().find(|row| row.owner == owner) {
            row.msat = row.msat.saturating_add(amount);
        } else {
            self.state.balances.push(BalanceEntry { owner, msat: amount });
        }
        self.persist()?;
        Ok(true)
    }

    pub fn settle_payout(&mut self, id: &str) -> Result<(), String> {
        let (owner, amount) = {
            let record = self
                .state
                .escrows
                .iter()
                .find(|record| record.id == id)
                .ok_or_else(|| "escrow not found".to_string())?;
            if record.payout != "buyer" && record.payout != "seller" {
                return Err("no owed payout".to_string());
            }
            let owner = if record.payout == "buyer" {
                record.owner.clone()
            } else {
                record.seller_owner.clone()
            };
            (owner, record.amount_msat)
        };
        let row = self
            .state
            .balances
            .iter_mut()
            .find(|row| row.owner == owner)
            .ok_or_else(|| "balance missing".to_string())?;
        if row.msat < amount {
            return Err("balance short".to_string());
        }
        row.msat -= amount;
        let record = self
            .state
            .escrows
            .iter_mut()
            .find(|record| record.id == id)
            .ok_or_else(|| "escrow not found".to_string())?;
        record.payout = "paid".to_string();
        self.persist()
    }

    pub fn append_event(&mut self, escrow_id: &str, event: NostrEvent) -> Result<String, String> {
        let chain = self
            .state
            .event_chains
            .entry(escrow_id.to_string())
            .or_default();
        chain.push(event.clone());
        let event_id = event.id.clone();
        let record = self
            .state
            .escrows
            .iter_mut()
            .find(|record| record.id == escrow_id)
            .ok_or_else(|| "escrow not found".to_string())?;
        record.tip_id = Some(event_id.clone());
        self.persist()?;
        Ok(event_id)
    }

    pub fn get_events(&self, escrow_id: &str) -> Vec<NostrEvent> {
        self.state
            .event_chains
            .get(escrow_id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn get_root_id(&self, escrow_id: &str) -> Option<String> {
        self.state
            .escrows
            .iter()
            .find(|record| record.id == escrow_id)
            .and_then(|record| record.root_id.clone())
    }

    pub fn get_tip_id(&self, escrow_id: &str) -> Option<String> {
        self.state
            .escrows
            .iter()
            .find(|record| record.id == escrow_id)
            .and_then(|record| record.tip_id.clone())
            .or_else(|| self.get_root_id(escrow_id))
    }

    fn persist(&self) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(&self.state).map_err(|e| e.to_string())?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, &bytes).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }
}

pub fn tree_contains_secret(dir: &Path, hex_secret: &str, raw_secret: &[u8]) -> bool {
    fn walk(path: &Path, hex_secret: &str, raw_secret: &[u8]) -> bool {
        let Ok(entries) = fs::read_dir(path) else {
            return false;
        };
        for entry in entries.flatten() {
            let child = entry.path();
            if child.is_dir() {
                if walk(&child, hex_secret, raw_secret) {
                    return true;
                }
                continue;
            }
            let Ok(bytes) = fs::read(&child) else {
                continue;
            };
            if bytes.windows(raw_secret.len()).any(|window| window == raw_secret) {
                return true;
            }
            if String::from_utf8_lossy(&bytes).contains(hex_secret) {
                return true;
            }
        }
        false
    }
    walk(dir, hex_secret, raw_secret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_are_isolated_and_claim_does_not_persist_preimage() {
        let dir = std::env::temp_dir().join(format!(
            "ponthtlc-tenancy-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut store = EscrowStore::open(dir.join("escrows.json")).unwrap();
        store
            .insert(EscrowRecord {
                id: "escrow-a".to_string(),
                owner: "owner-a".to_string(),
                payment_hash: "aa".to_string(),
                state: "open".to_string(),
                bolt11: String::new(),
                protocol_status: String::new(),
                root_id: None,
                tip_id: None,
                ..EscrowRecord::default()
            })
            .unwrap();
        store
            .insert(EscrowRecord {
                id: "escrow-b".to_string(),
                owner: "owner-b".to_string(),
                payment_hash: "bb".to_string(),
                state: "open".to_string(),
                bolt11: String::new(),
                protocol_status: String::new(),
                root_id: None,
                tip_id: None,
                ..EscrowRecord::default()
            })
            .unwrap();
        store.assign_channel("owner-a", "chan-a").unwrap();
        store.assign_channel("owner-b", "chan-b").unwrap();

        let listed = store.list("owner-a");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "escrow-a");
        assert!(!store.channels_for("owner-a").contains(&"chan-b".to_string()));
        assert!(store.channels_for("owner-b").contains(&"chan-b".to_string()));

        let preimage = [0x11u8; 32];
        let preimage_hex = hex::encode(preimage);
        store.mark_claim_submitted("escrow-a", "owner-a").unwrap();
        assert!(!tree_contains_secret(&dir, &preimage_hex, &preimage));
        assert_eq!(store.get("escrow-a").unwrap().state, "claim_submitted");
        store.record_outgoing("owner-a", "cc").unwrap();
        assert!(store.visible_hash("owner-a", "cc"));
        assert!(!store.visible_hash("owner-b", "cc"));
        store.mark_fail_submitted("escrow-b", "owner-b").unwrap();
        assert_eq!(store.get_owned("escrow-b", "owner-b").unwrap().state, "fail_submitted");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn legacy_escrow_file_loads_without_bolt11() {
        let dir = std::env::temp_dir().join(format!(
            "ponthtlc-tenancy-legacy-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("escrows.json");
        fs::write(
            &path,
            r#"{"escrows":[{"id":"escrow-old","owner":"owner-a","payment_hash":"aa","state":"open"}],"channels":[]}"#,
        )
        .unwrap();
        let store = EscrowStore::open(path).unwrap();
        let record = store.get_owned("escrow-old", "owner-a").unwrap();
        assert_eq!(record.bolt11, "");
        assert_eq!(record.state, "open");
        assert_eq!(record.buyer_lightning_address, "");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn owed_balance_survives_restart_until_payout() {
        let dir = std::env::temp_dir().join(format!(
            "ponthtlc-tenancy-balance-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("escrows.json");
        let mut store = EscrowStore::open(&path).unwrap();
        store
            .insert(EscrowRecord {
                id: "escrow-a".to_string(),
                owner: "buyer".to_string(),
                payment_hash: "aa".to_string(),
                state: "open".to_string(),
                amount_msat: 50_000,
                seller_owner: "seller".to_string(),
                seller_lightning_address: "seller@example.com".to_string(),
                ..EscrowRecord::default()
            })
            .unwrap();
        assert!(store.credit_payout("escrow-a", "seller").unwrap());
        assert!(!store.credit_payout("escrow-a", "seller").unwrap());
        assert_eq!(store.balance_of("seller"), 50_000);
        drop(store);
        let mut store = EscrowStore::open(&path).unwrap();
        assert_eq!(store.balance_of("seller"), 50_000);
        assert_eq!(store.owed().len(), 1);
        store.settle_payout("escrow-a").unwrap();
        assert_eq!(store.balance_of("seller"), 0);
        assert_eq!(store.get("escrow-a").unwrap().payout, "paid");
        let _ = fs::remove_dir_all(&dir);
    }
}
