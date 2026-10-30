use std::fmt;

use bitcoin::hashes::sha256::Hash as Sha256;
use bitcoin::hashes::Hash;
use rand::rngs::OsRng;
use secp256k1::schnorr;
use secp256k1::{Secp256k1, XOnlyPublicKey};
use secp256k1::rand::RngCore;

use crate::constants;

pub type PubkeyHex = String;
pub type EventId = String;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NostrEvent {
    pub id: EventId,
    pub pubkey: PubkeyHex,
    pub created_at: u64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
    pub sig: String,
}

#[derive(Clone, Debug)]
pub struct UnsignedEvent {
    pub pubkey: PubkeyHex,
    pub created_at: u64,
    pub kind: u32,
    pub tags: Vec<Vec<String>>,
    pub content: String,
}

impl UnsignedEvent {
    pub fn id(&self) -> [u8; 32] {
        let json = canonical_json(
            &self.pubkey,
            self.created_at,
            self.kind,
            &self.tags,
            &self.content,
        );
        Sha256::hash(json.as_bytes()).to_byte_array()
    }

    pub fn id_hex(&self) -> String {
        hex::encode(self.id())
    }

    pub fn sign(self, secret_key: &[u8; 32]) -> Result<NostrEvent, String> {
        let secp = Secp256k1::new();
        let secret =
            secp256k1::SecretKey::from_slice(secret_key).map_err(|e| format!("bad secret key: {e}"))?;
        let keypair = secp256k1::Keypair::from_secret_key(&secp, &secret);
        let (x_only, _) = keypair.x_only_public_key();
        let pubkey = hex::encode(x_only.serialize());
        let id = canonical_json(
            &pubkey,
            self.created_at,
            self.kind,
            &self.tags,
            &self.content,
        );
        let id_bytes = Sha256::hash(id.as_bytes()).to_byte_array();
        let msg = secp256k1::Message::from_slice(&id_bytes).map_err(|e| format!("bad message: {e}"))?;
        let mut aux = [0u8; 32];
        OsRng.fill_bytes(&mut aux);
        let sig = secp.sign_schnorr_with_aux_rand(&msg, &keypair, &aux);
        Ok(NostrEvent {
            id: hex::encode(id_bytes),
            pubkey,
            created_at: self.created_at,
            kind: self.kind,
            tags: self.tags,
            content: self.content,
            sig: sig.to_string(),
        })
    }
}

impl NostrEvent {
    pub fn verify(&self) -> bool {
        let Ok(pubkey_bytes) = hex::decode(&self.pubkey) else {
            return false;
        };
        let Ok(pubkey_arr) = <[u8; 32]>::try_from(&pubkey_bytes[..]) else {
            return false;
        };
        let Ok(x_only) = XOnlyPublicKey::from_slice(&pubkey_arr) else {
            return false;
        };
        let Ok(id_bytes) = hex::decode(&self.id) else {
            return false;
        };
        let Ok(id_arr) = <[u8; 32]>::try_from(&id_bytes[..]) else {
            return false;
        };
        let Ok(sig_bytes) = hex::decode(&self.sig) else {
            return false;
        };
        let Ok(sig) = schnorr::Signature::from_slice(&sig_bytes) else {
            return false;
        };
        let Ok(msg) = secp256k1::Message::from_slice(&id_arr) else {
            return false;
        };
        let secp = Secp256k1::new();
        secp.verify_schnorr(&sig, &msg, &x_only).is_ok()
    }

    pub fn tag(&self, name: &str) -> Option<&[String]> {
        self.tags
            .iter()
            .find(|t| t.first().map(|s| s.as_str()) == Some(name))
            .map(|v| v.as_slice())
    }

    pub fn tags_by_name(&self, name: &str) -> Vec<&[String]> {
        self.tags
            .iter()
            .filter(|t| t.first().map(|s| s.as_str()) == Some(name))
            .map(|v| v.as_slice())
            .collect()
    }

    pub fn tag_value(&self, name: &str) -> Option<&str> {
        self.tag(name).and_then(|t| t.get(1).map(|s| s.as_str()))
    }
}

fn canonical_json(
    pubkey: &str,
    created_at: u64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> String {
    let tags_json: Vec<String> = tags
        .iter()
        .map(|tag| {
            let inner: Vec<String> = tag.iter().map(|s| serde_json::to_string(s).unwrap()).collect();
            format!("[{}]", inner.join(","))
        })
        .collect();
    format!(
        "[0,\"{}\",{},{},[{}],{}]",
        pubkey,
        created_at,
        kind,
        tags_json.join(","),
        serde_json::to_string(content).unwrap()
    )
}

pub fn generate_keypair() -> ([u8; 32], String) {
    let secp = Secp256k1::new();
    let (secret, public) = secp.generate_keypair(&mut OsRng);
    let (x_only, _) = public.x_only_public_key();
    let mut secret_bytes = [0u8; 32];
    secret_bytes.copy_from_slice(&secret.secret_bytes()[..]);
    let pk_serialized: [u8; 32] = x_only.serialize();
    (secret_bytes, hex::encode(pk_serialized))
}

pub fn pubkey_from_secret(secret: &[u8; 32]) -> String {
    let secp = Secp256k1::new();
    let secret_key = secp256k1::SecretKey::from_slice(secret).expect("valid secret key");
    let keypair = secp256k1::Keypair::from_secret_key(&secp, &secret_key);
    let (x_only, _) = keypair.x_only_public_key();
    hex::encode(x_only.serialize())
}

pub fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock")
        .as_secs()
}

impl fmt::Display for NostrEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Event(id={}, kind={}, pubkey={:.8})", self.id, self.kind, self.pubkey)
    }
}

pub fn kind_name(kind: u32) -> &'static str {
    match kind {
        constants::KIND_AGENT_DEFINITION => "agent_definition",
        constants::KIND_ESCROW_DESCRIPTOR => "escrow_descriptor",
        constants::KIND_ROOT => "root",
        constants::KIND_ACTION => "action",
        constants::KIND_OFFER => "offer",
        constants::KIND_GIFT_WRAP => "gift_wrap",
        constants::KIND_CHAT => "chat",
        constants::KIND_SWAP_PAYLOAD => "swap_payload",
        _ => "unknown",
    }
}