use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::constants::{self, DISPUTE_CLASSES, PIP02_VERSION, SWAP_PROFILE};
use crate::event::{is_hex64, NostrEvent};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    FiatToBtc,
    BtcToFiat,
}

impl Direction {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "fiat_to_btc" => Some(Self::FiatToBtc),
            "btc_to_fiat" => Some(Self::BtcToFiat),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FiatToBtc => "fiat_to_btc",
            Self::BtcToFiat => "btc_to_fiat",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwapTerms {
    pub direction: Direction,
    pub fiat: FiatTerms,
    pub bitcoin: BitcoinTerms,
    pub payment_channel: String,
    pub deadlines: DeadlineTerms,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FiatTerms {
    pub currency: String,
    pub amount: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BitcoinTerms {
    pub amount: String,
    pub unit: String,
    pub network: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeadlineTerms {
    pub fiat_pay_by: u64,
    pub fiat_confirm_by: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitmentObject {
    pub algorithm: String,
    pub digest: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwapRoot {
    pub id: String,
    pub event: NostrEvent,
    pub created_at: u64,
    pub proposer: String,
    pub agent: String,
    pub customer: String,
    pub escrow: String,
    pub resolver: String,
    pub descriptor_id: String,
    pub descriptor_address: String,
    pub terms: SwapTerms,
    pub expires_at: u64,
    pub commitments: HashMap<String, CommitmentObject>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwapStatus {
    Proposed,
    Accepted,
    Secured,
    FiatSent,
    FiatConfirmed,
    SettlementAuthorized,
    RefundAuthorized,
    Settled,
    Refunded,
    Declined,
    Cancelled,
    Expired,
}

impl SwapStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Settled | Self::Refunded | Self::Declined | Self::Cancelled | Self::Expired
        )
    }
}

const SECURED_OR_LATER: &[SwapStatus] = &[
    SwapStatus::Secured,
    SwapStatus::FiatSent,
    SwapStatus::FiatConfirmed,
    SwapStatus::SettlementAuthorized,
    SwapStatus::RefundAuthorized,
];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppliedAction {
    pub id: String,
    pub action: String,
    pub signer: String,
    pub created_at: u64,
    pub data: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RejectedAction {
    pub id: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DisputeInfo {
    pub by: String,
    pub class: Option<String>,
    pub at: u64,
    pub pre_status: SwapStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolutionInfo {
    pub effect: String,
    pub at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwapState {
    pub root: SwapRoot,
    pub status: SwapStatus,
    pub disputed: bool,
    pub dispute: Option<DisputeInfo>,
    pub resolution: Option<ResolutionInfo>,
    pub forked: bool,
    pub fork_ids: Vec<String>,
    pub tip: String,
    pub chain: Vec<AppliedAction>,
    pub rejected: Vec<RejectedAction>,
    pub payment_reference: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct ProtocolError(pub &'static str, pub String);

impl ProtocolError {
    fn new(code: &'static str, msg: impl Into<String>) -> Self {
        ProtocolError(code, msg.into())
    }
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.0, self.1)
    }
}

pub fn fiat_sender(root: &SwapRoot) -> &str {
    match root.terms.direction {
        Direction::FiatToBtc => &root.customer,
        Direction::BtcToFiat => &root.agent,
    }
}

pub fn fiat_receiver(root: &SwapRoot) -> &str {
    match root.terms.direction {
        Direction::FiatToBtc => &root.agent,
        Direction::BtcToFiat => &root.customer,
    }
}

pub fn bitcoin_provider(root: &SwapRoot) -> &str {
    fiat_receiver(root)
}

pub fn bitcoin_recipient(root: &SwapRoot) -> &str {
    fiat_sender(root)
}

fn is_positive_decimal(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(|c| c.is_ascii_digit() || c == '.')
        && s.parse::<f64>().map(|v| v > 0.0).unwrap_or(false)
}

fn is_valid_ts(v: &serde_json::Value) -> Option<u64> {
    v.as_u64().filter(|t| *t > 0)
}

pub fn validate_terms(
    terms: &serde_json::Value,
    root_expires_at: u64,
) -> Result<SwapTerms, ProtocolError> {
    let obj = terms.as_object().ok_or(ProtocolError::new("terms_invalid", "Terms must be an object"))?;
    let direction = Direction::from_str(
        obj.get("direction")
            .and_then(|v| v.as_str())
            .ok_or(ProtocolError::new("terms_invalid", "direction required"))?,
    )
    .ok_or(ProtocolError::new("terms_invalid", "Unknown direction"))?;
    let fiat = obj
        .get("fiat")
        .and_then(|v| v.as_object())
        .ok_or(ProtocolError::new("terms_invalid", "fiat required"))?;
    let currency = fiat
        .get("currency")
        .and_then(|v| v.as_str())
        .ok_or(ProtocolError::new("terms_invalid", "fiat currency required"))?;
    if !currency.chars().all(|c| c.is_ascii_uppercase()) || currency.len() != 3 {
        return Err(ProtocolError::new("terms_invalid", "Fiat currency must be ISO 4217"));
    }
    let amount = fiat
        .get("amount")
        .and_then(|v| v.as_str())
        .ok_or(ProtocolError::new("terms_invalid", "fiat amount required as string"))?;
    if !is_positive_decimal(amount) {
        return Err(ProtocolError::new("terms_invalid", "Fiat amount must be a positive decimal string"));
    }
    let bitcoin = obj
        .get("bitcoin")
        .and_then(|v| v.as_object())
        .ok_or(ProtocolError::new("terms_invalid", "bitcoin required"))?;
    let btc_amount = bitcoin
        .get("amount")
        .and_then(|v| v.as_str())
        .ok_or(ProtocolError::new("terms_invalid", "bitcoin amount required as string"))?;
    if !btc_amount.chars().all(|c| c.is_ascii_digit()) || btc_amount == "0" {
        return Err(ProtocolError::new("terms_invalid", "Bitcoin amount must be a positive integer string"));
    }
    let unit = bitcoin
        .get("unit")
        .and_then(|v| v.as_str())
        .ok_or(ProtocolError::new("terms_invalid", "bitcoin unit required"))?;
    if unit != "sat" {
        return Err(ProtocolError::new("terms_invalid", "Bitcoin unit must be sat"));
    }
    let network = bitcoin
        .get("network")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or(ProtocolError::new("terms_invalid", "Bitcoin network required"))?;
    let payment_channel = obj
        .get("payment_channel")
        .and_then(|v| v.as_str())
        .ok_or(ProtocolError::new("terms_invalid", "payment_channel required"))?;
    if !payment_channel.contains('@') {
        return Err(ProtocolError::new("terms_invalid", "Payment channel must be versioned"));
    }
    let deadlines = obj
        .get("deadlines")
        .and_then(|v| v.as_object())
        .ok_or(ProtocolError::new("terms_invalid", "deadlines required"))?;
    let fiat_pay_by = is_valid_ts(deadlines.get("fiat_pay_by").unwrap_or(&serde_json::Value::Null))
        .ok_or(ProtocolError::new("terms_invalid", "fiat_pay_by required"))?;
    let fiat_confirm_by =
        is_valid_ts(deadlines.get("fiat_confirm_by").unwrap_or(&serde_json::Value::Null))
            .ok_or(ProtocolError::new("terms_invalid", "fiat_confirm_by required"))?;
    if !(root_expires_at < fiat_pay_by && fiat_pay_by < fiat_confirm_by) {
        return Err(ProtocolError::new(
            "terms_invalid",
            "Deadlines must satisfy expires_at < fiat_pay_by < fiat_confirm_by",
        ));
    }
    Ok(SwapTerms {
        direction,
        fiat: FiatTerms {
            currency: currency.to_string(),
            amount: amount.to_string(),
        },
        bitcoin: BitcoinTerms {
            amount: btc_amount.to_string(),
            unit: "sat".to_string(),
            network: network.to_string(),
        },
        payment_channel: payment_channel.to_string(),
        deadlines: DeadlineTerms {
            fiat_pay_by,
            fiat_confirm_by,
        },
    })
}

pub fn parse_root(event: &NostrEvent, verify: bool) -> Result<SwapRoot, ProtocolError> {
    if event.kind != constants::KIND_ROOT {
        return Err(ProtocolError::new("root_kind", "Not a coordination root"));
    }
    if verify && !event.verify() {
        return Err(ProtocolError::new("signature", "Invalid root signature"));
    }
    let content: serde_json::Value =
        serde_json::from_str(&event.content).map_err(|_| ProtocolError::new("root_content", "Root content is not JSON"))?;
    let obj = content
        .as_object()
        .ok_or(ProtocolError::new("root_content", "Root content must be an object"))?;
    if obj.get("version").and_then(|v| v.as_u64()) != Some(PIP02_VERSION as u64) {
        return Err(ProtocolError::new("version", "Unsupported PIP-02 version"));
    }
    if obj.get("profile").and_then(|v| v.as_str()) != Some(SWAP_PROFILE) {
        return Err(ProtocolError::new("profile", "Unsupported profile"));
    }
    let expires_at = is_valid_ts(obj.get("expires_at").unwrap_or(&serde_json::Value::Null))
        .ok_or(ProtocolError::new("root_content", "expires_at required"))?;

    let mut roles: HashMap<String, Vec<String>> = HashMap::new();
    for tag in &event.tags {
        if tag.first().map(|s| s.as_str()) != Some("p") {
            continue;
        }
        let role = tag.get(3).cloned();
        let pk = tag.get(1).cloned();
        if let (Some(role), Some(pk)) = (role, pk) {
            if !is_hex64(&pk) {
                return Err(ProtocolError::new("roles", "Invalid participant pubkey"));
            }
            roles.entry(role).or_default().push(pk);
        }
    }
    let one_role = |role: &str| -> Result<String, ProtocolError> {
        let pks = roles.get(role);
        match pks {
            Some(pks) if pks.len() == 1 => Ok(pks[0].clone()),
            _ => Err(ProtocolError::new("roles", format!("Root must bind exactly one {role}"))),
        }
    };
    let agent = one_role("swap/agent")?;
    let customer = one_role("swap/customer")?;
    let escrow = one_role("core/escrow")?;
    let resolver = one_role("core/resolver")?;
    let all = vec![&agent, &customer, &escrow, &resolver];
    let unique: HashSet<&String> = all.iter().copied().collect();
    if unique.len() != all.len() {
        return Err(ProtocolError::new(
            "roles",
            "Profile does not permit one pubkey to hold several roles",
        ));
    }
    if event.pubkey != agent && event.pubkey != customer {
        return Err(ProtocolError::new("roles", "Root signer must be an application participant"));
    }

    let desc_e: Vec<&Vec<String>> = event
        .tags
        .iter()
        .filter(|t| t.first().map(|s| s.as_str()) == Some("e") && t.get(3).map(|s| s.as_str()) == Some("escrow-version"))
        .collect();
    let desc_a: Vec<&Vec<String>> = event
        .tags
        .iter()
        .filter(|t| t.first().map(|s| s.as_str()) == Some("a") && t.get(3).map(|s| s.as_str()) == Some("escrow"))
        .collect();
    if desc_e.len() != 1 || !desc_e[0].get(1).map(|s| is_hex64(s)).unwrap_or(false) {
        return Err(ProtocolError::new("escrow_binding", "Root must bind one exact escrow descriptor"));
    }
    let desc_a_val = desc_a
        .get(0)
        .and_then(|t| t.get(1))
        .ok_or(ProtocolError::new("escrow_binding", "Root must bind one escrow descriptor address"))?;
    if !desc_a_val.starts_with("30361:") {
        return Err(ProtocolError::new("escrow_binding", "Root must bind one escrow descriptor address"));
    }

    let terms = validate_terms(obj.get("terms").unwrap_or(&serde_json::Value::Null), expires_at)?;

    let commitments: HashMap<String, CommitmentObject> = if let Some(c) = obj.get("commitments") {
        let c_obj = c
            .as_object()
            .ok_or(ProtocolError::new("commitments", "commitments must be an object"))?;
        let mut map = HashMap::new();
        for (key, value) in c_obj {
            if key != "quote" && key != "private_terms" {
                return Err(ProtocolError::new("commitments", format!("Commitment key {key} not permitted")));
            }
            let v = value.as_object().ok_or(ProtocolError::new("commitments", "Invalid commitment"))?;
            let algo = v.get("algorithm").and_then(|v| v.as_str()).unwrap_or("");
            if algo != "sha256-bytes@1" {
                return Err(ProtocolError::new("commitments", "Commitments must use sha256-bytes@1"));
            }
            let digest = v.get("digest").and_then(|v| v.as_str()).unwrap_or("");
            if !digest.starts_with("sha256:") || !is_hex64(&digest[7..]) {
                return Err(ProtocolError::new("commitments", "Invalid commitment digest"));
            }
            map.insert(
                key.clone(),
                CommitmentObject {
                    algorithm: algo.to_string(),
                    digest: digest.to_string(),
                },
            );
        }
        map
    } else {
        HashMap::new()
    };

    Ok(SwapRoot {
        id: event.id.clone(),
        event: event.clone(),
        created_at: event.created_at,
        proposer: event.pubkey.clone(),
        agent,
        customer,
        escrow,
        resolver,
        descriptor_id: desc_e[0][1].clone(),
        descriptor_address: desc_a_val.clone(),
        terms,
        expires_at,
        commitments,
    })
}

#[derive(Clone)]
struct ParsedAction {
    event: NostrEvent,
    prev: String,
    action: String,
    data: Option<serde_json::Value>,
}

enum ParseActionResult {
    Ok(ParsedAction),
    Skip,
    Reject(String),
}

fn parse_action(event: &NostrEvent, root_id: &str, verify: bool) -> ParseActionResult {
    if event.kind != constants::KIND_ACTION {
        return ParseActionResult::Skip;
    }
    if verify && !event.verify() {
        return ParseActionResult::Reject("invalid signature".to_string());
    }
    let roots: Vec<&Vec<String>> = event
        .tags
        .iter()
        .filter(|t| t.first().map(|s| s.as_str()) == Some("e") && t.get(3).map(|s| s.as_str()) == Some("root"))
        .collect();
    let prevs: Vec<&Vec<String>> = event
        .tags
        .iter()
        .filter(|t| t.first().map(|s| s.as_str()) == Some("e") && t.get(3).map(|s| s.as_str()) == Some("prev"))
        .collect();
    if roots.len() != 1 || prevs.len() != 1 {
        return ParseActionResult::Reject("must reference exactly one root and one prev".to_string());
    }
    let root_ref = roots[0].get(1).map(|s| s.as_str()).unwrap_or("");
    if root_ref != root_id {
        return ParseActionResult::Skip;
    }
    let content: serde_json::Value = match serde_json::from_str(&event.content) {
        Ok(c) => c,
        Err(_) => return ParseActionResult::Reject("content is not JSON".to_string()),
    };
    let obj = match content.as_object() {
        Some(o) => o,
        None => return ParseActionResult::Reject("invalid action content".to_string()),
    };
    if obj.get("version").and_then(|v| v.as_u64()) != Some(PIP02_VERSION as u64) {
        return ParseActionResult::Reject("unsupported PIP-02 version".to_string());
    }
    let action = match obj.get("action").and_then(|v| v.as_str()) {
        Some(a) => a.to_string(),
        None => return ParseActionResult::Reject("action required".to_string()),
    };
    let data = obj.get("data").cloned();
    if let Some(d) = &data {
        if !d.is_object() {
            return ParseActionResult::Reject("data must be an object".to_string());
        }
    }
    ParseActionResult::Ok(ParsedAction {
        event: event.clone(),
        prev: prevs[0].get(1).cloned().unwrap_or_default(),
        action,
        data,
    })
}

fn kernel_data_ok(data: &Option<serde_json::Value>, allowed: &[&str]) -> bool {
    match data {
        None => true,
        Some(d) => {
            let obj = match d.as_object() {
                Some(o) => o,
                None => return false,
            };
            for key in obj.keys() {
                if key != "evidence" && !allowed.contains(&key.as_str()) {
                    return false;
                }
            }
            if let Some(ev) = obj.get("evidence") {
                if !valid_evidence(ev) {
                    return false;
                }
            }
            true
        }
    }
}

fn valid_evidence(value: &serde_json::Value) -> bool {
    value.as_array()
        .map(|arr| {
            arr.iter().all(|r| {
                r.as_object()
                    .map(|o| {
                        let t = o.get("type").and_then(|v| v.as_str());
                        let val = o.get("value").and_then(|v| v.as_str());
                        matches!(t, Some("event") | Some("commitment") | Some("opaque")) && val.is_some()
                    })
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn valid_payment_reference(ref_val: &serde_json::Value) -> bool {
    if let Some(s) = ref_val.as_str() {
        return !s.is_empty()
            && s.len() <= 128
            && s.chars().all(|c| c.is_ascii_alphanumeric() || ".:-_".contains(c));
    }
    if let Some(obj) = ref_val.as_object() {
        let algo = obj.get("algorithm").and_then(|v| v.as_str()).unwrap_or("");
        let digest = obj.get("digest").and_then(|v| v.as_str()).unwrap_or("");
        return algo == "sha256-bytes@1" && digest.starts_with("sha256:") && is_hex64(&digest[7..]);
    }
    false
}

fn same_payment_reference(a: &serde_json::Value, b: &Option<serde_json::Value>) -> bool {
    match (a, b) {
        (av, Some(bv)) => {
            if av.is_string() || bv.is_string() {
                av == bv
            } else {
                if let (Some(ao), Some(bo)) = (av.as_object(), bv.as_object()) {
                    ao.get("digest") == bo.get("digest")
                } else {
                    false
                }
            }
        }
        _ => false,
    }
}

enum StepResult {
    Ok(SwapState),
    Reject(String),
}

fn step(state: &SwapState, a: &ParsedAction) -> StepResult {
    let root = &state.root;
    let signer = &a.event.pubkey;
    let t = a.event.created_at;
    let is_participant = signer == &root.agent || signer == &root.customer;
    let non_proposer = if root.proposer == root.agent {
        &root.customer
    } else {
        &root.agent
    };
    let fiat_pay_by = root.terms.deadlines.fiat_pay_by;
    let fiat_confirm_by = root.terms.deadlines.fiat_confirm_by;

    if state.status.is_terminal() {
        return StepResult::Reject("coordination is terminal".to_string());
    }
    if state.disputed && a.action != "core/resolve_dispute" {
        return StepResult::Reject("disputed: ordinary progress frozen".to_string());
    }

    match a.action.as_str() {
        "core/accept" => {
            if !kernel_data_ok(&a.data, &[]) {
                return StepResult::Reject("invalid data".to_string());
            }
            if signer != non_proposer {
                return StepResult::Reject("only the non-proposing participant accepts".to_string());
            }
            if state.status != SwapStatus::Proposed {
                return StepResult::Reject("not awaiting acceptance".to_string());
            }
            if t >= root.expires_at {
                return StepResult::Reject("accepted after expires_at".to_string());
            }
            StepResult::Ok(SwapState {
                status: SwapStatus::Accepted,
                ..state.clone()
            })
        }
        "core/decline" => {
            if !kernel_data_ok(&a.data, &[]) {
                return StepResult::Reject("invalid data".to_string());
            }
            if signer != non_proposer {
                return StepResult::Reject("only the non-proposing participant declines".to_string());
            }
            if state.status != SwapStatus::Proposed {
                return StepResult::Reject("not awaiting acceptance".to_string());
            }
            StepResult::Ok(SwapState {
                status: SwapStatus::Declined,
                ..state.clone()
            })
        }
        "core/cancel" => {
            if !kernel_data_ok(&a.data, &[]) {
                return StepResult::Reject("invalid data".to_string());
            }
            if state.status == SwapStatus::Proposed && signer == &root.proposer {
                return StepResult::Ok(SwapState {
                    status: SwapStatus::Cancelled,
                    ..state.clone()
                });
            }
            if state.status == SwapStatus::Accepted && is_participant {
                return StepResult::Ok(SwapState {
                    status: SwapStatus::Cancelled,
                    ..state.clone()
                });
            }
            StepResult::Reject("cancel not permitted now".to_string())
        }
        "core/expire" => {
            if !kernel_data_ok(&a.data, &[]) {
                return StepResult::Reject("invalid data".to_string());
            }
            if !is_participant {
                return StepResult::Reject("only participants record expiry".to_string());
            }
            if state.status != SwapStatus::Proposed {
                return StepResult::Reject("only an unaccepted swap expires".to_string());
            }
            if t < root.expires_at {
                return StepResult::Reject("not yet expired".to_string());
            }
            StepResult::Ok(SwapState {
                status: SwapStatus::Expired,
                ..state.clone()
            })
        }
        "core/secure" => {
            if !kernel_data_ok(&a.data, &[]) {
                return StepResult::Reject("invalid data".to_string());
            }
            if signer != &root.escrow {
                return StepResult::Reject("only the bound escrow secures".to_string());
            }
            if state.status != SwapStatus::Accepted {
                return StepResult::Reject("not accepted".to_string());
            }
            StepResult::Ok(SwapState {
                status: SwapStatus::Secured,
                ..state.clone()
            })
        }
        "swap/fiat_sent" => {
            if signer != fiat_sender(root) {
                return StepResult::Reject("only the fiat sender records fiat_sent".to_string());
            }
            if state.status != SwapStatus::Secured {
                return StepResult::Reject("not secured".to_string());
            }
            if t >= fiat_pay_by {
                return StepResult::Reject("after fiat_pay_by".to_string());
            }
            let ref_val = a
                .data
                .as_ref()
                .and_then(|d| d.get("payment_reference"))
                .unwrap_or(&serde_json::Value::Null);
            if !valid_payment_reference(ref_val) {
                return StepResult::Reject("invalid payment_reference".to_string());
            }
            StepResult::Ok(SwapState {
                status: SwapStatus::FiatSent,
                payment_reference: Some(ref_val.clone()),
                ..state.clone()
            })
        }
        "swap/fiat_confirmed" => {
            if signer != fiat_receiver(root) {
                return StepResult::Reject("only the fiat receiver confirms".to_string());
            }
            if state.status != SwapStatus::FiatSent {
                return StepResult::Reject("no fiat_sent to confirm".to_string());
            }
            if t >= fiat_confirm_by {
                return StepResult::Reject("after fiat_confirm_by".to_string());
            }
            let ref_val = a
                .data
                .as_ref()
                .and_then(|d| d.get("payment_reference"))
                .unwrap_or(&serde_json::Value::Null);
            if !same_payment_reference(ref_val, &state.payment_reference) {
                return StepResult::Reject("payment_reference mismatch".to_string());
            }
            StepResult::Ok(SwapState {
                status: SwapStatus::FiatConfirmed,
                ..state.clone()
            })
        }
        "core/authorize_settlement" => {
            if !kernel_data_ok(&a.data, &[]) {
                return StepResult::Reject("invalid data".to_string());
            }
            if signer != fiat_receiver(root) {
                return StepResult::Reject("only the fiat receiver authorizes settlement".to_string());
            }
            if state.status != SwapStatus::FiatConfirmed {
                return StepResult::Reject("fiat not confirmed".to_string());
            }
            StepResult::Ok(SwapState {
                status: SwapStatus::SettlementAuthorized,
                ..state.clone()
            })
        }
        "core/settle" => {
            if !kernel_data_ok(&a.data, &[]) {
                return StepResult::Reject("invalid data".to_string());
            }
            if signer != &root.escrow {
                return StepResult::Reject("only the bound escrow settles".to_string());
            }
            if state.status != SwapStatus::SettlementAuthorized {
                return StepResult::Reject("settlement not authorized".to_string());
            }
            StepResult::Ok(SwapState {
                status: SwapStatus::Settled,
                ..state.clone()
            })
        }
        "core/authorize_refund" => {
            if !kernel_data_ok(&a.data, &[]) {
                return StepResult::Reject("invalid data".to_string());
            }
            if signer != bitcoin_provider(root) {
                return StepResult::Reject("only the bitcoin provider authorizes refund".to_string());
            }
            if state.status != SwapStatus::Secured {
                return StepResult::Reject("refund only from secured without fiat_sent".to_string());
            }
            if t < fiat_pay_by {
                return StepResult::Reject("fiat_pay_by has not elapsed".to_string());
            }
            StepResult::Ok(SwapState {
                status: SwapStatus::RefundAuthorized,
                ..state.clone()
            })
        }
        "core/refund" => {
            if !kernel_data_ok(&a.data, &[]) {
                return StepResult::Reject("invalid data".to_string());
            }
            if signer != &root.escrow {
                return StepResult::Reject("only the bound escrow refunds".to_string());
            }
            if state.status != SwapStatus::RefundAuthorized {
                return StepResult::Reject("refund not authorized".to_string());
            }
            StepResult::Ok(SwapState {
                status: SwapStatus::Refunded,
                ..state.clone()
            })
        }
        "core/open_dispute" => {
            if !kernel_data_ok(&a.data, &["class"]) {
                return StepResult::Reject("invalid data".to_string());
            }
            if !is_participant {
                return StepResult::Reject("only participants open disputes".to_string());
            }
            if state.status == SwapStatus::Proposed {
                return StepResult::Reject("not accepted".to_string());
            }
            let cls = a.data.as_ref().and_then(|d| d.get("class")).and_then(|v| v.as_str());
            if let Some(c) = cls {
                if !DISPUTE_CLASSES.contains(&c) {
                    return StepResult::Reject("unknown dispute class".to_string());
                }
            }
            StepResult::Ok(SwapState {
                disputed: true,
                dispute: Some(DisputeInfo {
                    by: signer.clone(),
                    class: cls.map(|s| s.to_string()),
                    at: t,
                    pre_status: state.status.clone(),
                }),
                ..state.clone()
            })
        }
        "core/resolve_dispute" => {
            if !kernel_data_ok(&a.data, &["policy", "effect"]) {
                return StepResult::Reject("invalid data".to_string());
            }
            if signer != &root.resolver {
                return StepResult::Reject("only the bound resolver resolves".to_string());
            }
            if !state.disputed || state.dispute.is_none() {
                return StepResult::Reject("not disputed".to_string());
            }
            let data_obj = match a.data.as_ref().and_then(|d| d.as_object()) {
                Some(o) => o,
                None => return StepResult::Reject("data required".to_string()),
            };
            let policy = data_obj.get("policy").and_then(|v| v.as_str()).unwrap_or("");
            if policy.is_empty() {
                return StepResult::Reject("policy required".to_string());
            }
            let effect = data_obj.get("effect").and_then(|v| v.as_str()).unwrap_or("");
            let pre = state.dispute.as_ref().map(|d| d.pre_status.clone()).unwrap_or(SwapStatus::Proposed);
            let base = SwapState {
                disputed: false,
                resolution: Some(ResolutionInfo {
                    effect: effect.to_string(),
                    at: t,
                }),
                ..state.clone()
            };
            match effect {
                "resume" => StepResult::Ok(SwapState {
                    status: pre,
                    ..base
                }),
                "authorize_settlement" => {
                    if !SECURED_OR_LATER.contains(&pre) {
                        return StepResult::Reject("no settlement before secure".to_string());
                    }
                    StepResult::Ok(SwapState {
                        status: SwapStatus::SettlementAuthorized,
                        ..base
                    })
                }
                "authorize_refund" => {
                    if !SECURED_OR_LATER.contains(&pre) {
                        return StepResult::Reject("no refund before secure".to_string());
                    }
                    StepResult::Ok(SwapState {
                        status: SwapStatus::RefundAuthorized,
                        ..base
                    })
                }
                "cancel" => StepResult::Ok(SwapState {
                    status: SwapStatus::Cancelled,
                    ..base
                }),
                _ => StepResult::Reject("unknown resolution effect".to_string()),
            }
        }
        other => {
            if other.starts_with("core/") {
                StepResult::Reject("unknown kernel action".to_string())
            } else {
                StepResult::Reject("action not defined by pontmore/swap@1".to_string())
            }
        }
    }
}

pub fn reconstruct(
    root: &SwapRoot,
    events: &[NostrEvent],
    verify: bool,
) -> SwapState {
    let mut state = SwapState {
        root: root.clone(),
        status: SwapStatus::Proposed,
        disputed: false,
        dispute: None,
        resolution: None,
        forked: false,
        fork_ids: Vec::new(),
        tip: root.id.clone(),
        chain: Vec::new(),
        rejected: Vec::new(),
        payment_reference: None,
    };

    let mut by_prev: HashMap<String, Vec<ParsedAction>> = HashMap::new();
    let mut seen: HashSet<String> = HashSet::new();

    for ev in events {
        if seen.contains(&ev.id) {
            continue;
        }
        seen.insert(ev.id.clone());
        match parse_action(ev, &root.id, verify) {
            ParseActionResult::Ok(parsed) => {
                by_prev.entry(parsed.prev.clone()).or_default().push(parsed);
            }
            ParseActionResult::Skip => {}
            ParseActionResult::Reject(reason) => {
                state.rejected.push(RejectedAction {
                    id: ev.id.clone(),
                    reason,
                });
            }
        }
    }

    loop {
        let children = by_prev.get(&state.tip);
        let children = match children {
            Some(c) => c,
            None => break,
        };
        let mut valid: Vec<(ParsedAction, SwapState)> = Vec::new();
        for child in children {
            match step(&state, child) {
                StepResult::Ok(next) => valid.push((child.clone(), next)),
                StepResult::Reject(reason) => {
                    state.rejected.push(RejectedAction {
                        id: child.event.id.clone(),
                        reason,
                    });
                }
            }
        }
        if valid.is_empty() {
            break;
        }
        if valid.len() > 1 {
            state.forked = true;
            state.fork_ids = valid.iter().map(|(a, _)| a.event.id.clone()).collect();
            state.fork_ids.sort();
            break;
        }
        let (a, next) = valid.into_iter().next().unwrap();
        state = SwapState {
            tip: a.event.id.clone(),
            chain: {
                let mut chain = state.chain.clone();
                chain.push(AppliedAction {
                    id: a.event.id.clone(),
                    action: a.action.clone(),
                    signer: a.event.pubkey.clone(),
                    created_at: a.event.created_at,
                    data: a.data.clone(),
                });
                chain
            },
            rejected: state.rejected.clone(),
            ..next
        };
    }

    state
}

pub fn is_lapsed(state: &SwapState, now: u64) -> bool {
    state.status == SwapStatus::Proposed && now >= state.root.expires_at
}

pub fn find_action<'a>(state: &'a SwapState, action: &str) -> Option<&'a AppliedAction> {
    state.chain.iter().find(|a| a.action == action)
}