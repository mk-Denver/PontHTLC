use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EscrowRecord {
    pub id: String,
    pub owner: String,
    pub payment_hash: String,
    pub state: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ChannelAssignment {
    owner: String,
    channel_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct DiskState {
    escrows: Vec<EscrowRecord>,
    channels: Vec<ChannelAssignment>,
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

    pub fn get(&self, id: &str) -> Option<EscrowRecord> {
        self.state.escrows.iter().find(|record| record.id == id).cloned()
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
        self.persist()
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
            })
            .unwrap();
        store
            .insert(EscrowRecord {
                id: "escrow-b".to_string(),
                owner: "owner-b".to_string(),
                payment_hash: "bb".to_string(),
                state: "open".to_string(),
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
        let _ = fs::remove_dir_all(&dir);
    }
}
