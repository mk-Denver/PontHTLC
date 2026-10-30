use serde::{Deserialize, Serialize};

pub const KIND_PROFILE: u32 = 0;
pub const KIND_RELAY_LIST: u32 = 10002;
pub const KIND_AGENT_DEFINITION: u32 = 30360;
pub const KIND_ESCROW_DESCRIPTOR: u32 = 30361;
pub const KIND_ROOT: u32 = 7300;
pub const KIND_ACTION: u32 = 7301;
pub const KIND_OFFER: u32 = 38383;
pub const KIND_GIFT_WRAP: u32 = 1059;
pub const KIND_SEAL: u32 = 13;
pub const KIND_CHAT: u32 = 14;
pub const KIND_SWAP_PAYLOAD: u32 = 7310;

pub const SWAP_PROFILE: &str = "pontmore/swap@1";
pub const SWAP_CAPABILITY: &str = "pontmore/swap@1";
pub const PIP02_VERSION: u32 = 2;

pub const ESCROW_TYPE: &str = "lightning_hold_invoice";
pub const ESCROW_D_TAG: &str = "lightning-hold-invoice";
pub const ESCROW_NETWORK: &str = "lightning";

pub const AGENT_D_TAG: &str = "agent";
pub const OFFER_PLATFORM: &str = "pontmore";
pub const RESOLUTION_POLICY: &str = "pontmore-mobile/agent-resolver@1";

pub fn capability_tag(id: &str) -> String {
    format!("pontmore-capability:{id}")
}

pub fn escrow_address(escrow_pk: &str) -> String {
    format!("{KIND_ESCROW_DESCRIPTOR}:{escrow_pk}:{ESCROW_D_TAG}")
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisputeClass {
    #[serde(rename = "fiat_not_received")]
    FiatNotReceived,
    #[serde(rename = "incorrect_fiat_amount")]
    IncorrectFiatAmount,
    #[serde(rename = "payment_reference_invalid")]
    PaymentReferenceInvalid,
    #[serde(rename = "escrow_not_secured")]
    EscrowNotSecured,
    #[serde(rename = "bitcoin_not_released")]
    BitcoinNotReleased,
    #[serde(rename = "conflicting_confirmation")]
    ConflictingConfirmation,
    #[serde(rename = "timeout")]
    Timeout,
}

impl DisputeClass {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "fiat_not_received" => Some(Self::FiatNotReceived),
            "incorrect_fiat_amount" => Some(Self::IncorrectFiatAmount),
            "payment_reference_invalid" => Some(Self::PaymentReferenceInvalid),
            "escrow_not_secured" => Some(Self::EscrowNotSecured),
            "bitcoin_not_released" => Some(Self::BitcoinNotReleased),
            "conflicting_confirmation" => Some(Self::ConflictingConfirmation),
            "timeout" => Some(Self::Timeout),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FiatNotReceived => "fiat_not_received",
            Self::IncorrectFiatAmount => "incorrect_fiat_amount",
            Self::PaymentReferenceInvalid => "payment_reference_invalid",
            Self::EscrowNotSecured => "escrow_not_secured",
            Self::BitcoinNotReleased => "bitcoin_not_released",
            Self::ConflictingConfirmation => "conflicting_confirmation",
            Self::Timeout => "timeout",
        }
    }
}

pub const DISPUTE_CLASSES: &[&str] = &[
    "fiat_not_received",
    "incorrect_fiat_amount",
    "payment_reference_invalid",
    "escrow_not_secured",
    "bitcoin_not_released",
    "conflicting_confirmation",
    "timeout",
];

pub mod timing {
    pub const ACCEPT_WINDOW: u64 = 15 * 60;
    pub const PAY_WINDOW: u64 = 75 * 60;
    pub const CONFIRM_WINDOW: u64 = 3 * 60 * 60;
    pub const HTLC_GRACE: u64 = 60 * 60;
    pub const OFFER_LIFETIME: u64 = 30 * 60;
    pub const QUOTE_TOLERANCE_PCT: u64 = 1;
    pub const DESCRIPTOR_LIFETIME: u64 = 365 * 24 * 60 * 60;
}