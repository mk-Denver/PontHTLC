use serde::{Deserialize, Serialize};

use ponthtlc_protocol::constants::SWAP_PROFILE;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaymentDetails {
    pub channel: String,
    pub details: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrivateTerms {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spark_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payout: Option<PaymentDetails>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PayloadBody {
    #[serde(rename = "request")]
    Request {
        quote: String,
        private_terms: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        payment_hash: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<String>,
    },
    #[serde(rename = "accept")]
    Accept {
        #[serde(skip_serializing_if = "Option::is_none")]
        payment: Option<PaymentDetails>,
        #[serde(skip_serializing_if = "Option::is_none")]
        spark_address: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        payment_hash: Option<String>,
    },
    #[serde(rename = "invoice")]
    Invoice { bolt11: String },
    #[serde(rename = "locked")]
    Locked {
        payment_hash: String,
        expires_at: u64,
        amount: String,
    },
    #[serde(rename = "fiat_sent")]
    FiatSent { reference: String },
    #[serde(rename = "share")]
    Share {
        x: u8,
        bytes: String,
    },
    #[serde(rename = "release")]
    Release { preimage: String },
    #[serde(rename = "claimed")]
    Claimed,
    #[serde(rename = "payout_invoice")]
    PayoutInvoice {
        bolt11: String,
        payment_hash: String,
    },
    #[serde(rename = "declined")]
    Declined { reasons: Vec<String> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwapPayload {
    #[serde(flatten)]
    pub body: PayloadBody,
    pub v: u32,
    pub profile: String,
    pub coordination: String,
    pub participants: Vec<String>,
    pub commitment_scheme: String,
}

impl SwapPayload {
    pub fn new(
        coordination: &str,
        participants: [String; 2],
        body: PayloadBody,
    ) -> Self {
        Self {
            body,
            v: 1,
            profile: SWAP_PROFILE.to_string(),
            coordination: coordination.to_string(),
            participants: participants.to_vec(),
            commitment_scheme: "sha256-bytes@1".to_string(),
        }
    }

    pub fn body_type(&self) -> &'static str {
        match &self.body {
            PayloadBody::Request { .. } => "request",
            PayloadBody::Accept { .. } => "accept",
            PayloadBody::Invoice { .. } => "invoice",
            PayloadBody::Locked { .. } => "locked",
            PayloadBody::FiatSent { .. } => "fiat_sent",
            PayloadBody::Share { .. } => "share",
            PayloadBody::Release { .. } => "release",
            PayloadBody::Claimed { .. } => "claimed",
            PayloadBody::PayoutInvoice { .. } => "payout_invoice",
            PayloadBody::Declined { .. } => "declined",
        }
    }
}

pub fn payment_reference_bytes(coordination: &str, reference: &str, payer: Option<&str>) -> String {
    let mut map = serde_json::Map::new();
    map.insert("coordination".to_string(), serde_json::Value::String(coordination.to_string()));
    map.insert("reference".to_string(), serde_json::Value::String(reference.trim().to_string()));
    if let Some(p) = payer.map(|p| p.trim()).filter(|p| !p.is_empty()) {
        map.insert("payer".to_string(), serde_json::Value::String(p.to_string()));
    }
    serde_json::Value::Object(map).to_string()
}

#[derive(Clone, Debug)]
pub struct InboxItem {
    pub id: String,
    pub from: String,
    pub to: String,
    pub created_at: u64,
    pub coordination: String,
    pub kind: InboxKind,
    pub text: Option<String>,
    pub payload: Option<SwapPayload>,
    pub raw_event: Option<ponthtlc_protocol::event::NostrEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InboxKind {
    Chat,
    Payload,
}