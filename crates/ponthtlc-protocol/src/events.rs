use std::collections::HashMap;

use crate::constants;
use crate::constants::{OFFER_PLATFORM, SWAP_PROFILE};
use crate::event::{NostrEvent, UnsignedEvent};
use crate::swap_v1::{CommitmentObject, SwapTerms};

pub fn agent_definition_template(escrow_pk: &str, relay_hint: &str) -> UnsignedEvent {
    UnsignedEvent {
        pubkey: escrow_pk.to_string(),
        created_at: crate::event::now(),
        kind: constants::KIND_AGENT_DEFINITION,
        tags: vec![
            vec!["d".to_string(), constants::AGENT_D_TAG.to_string()],
            vec!["t".to_string(), "agent".to_string()],
            vec![
                "t".to_string(),
                constants::capability_tag(constants::SWAP_CAPABILITY),
            ],
            vec![
                "a".to_string(),
                constants::escrow_address(escrow_pk),
                relay_hint.to_string(),
                "escrow".to_string(),
            ],
        ],
        content: serde_json::json!({
            "version": 1,
            "capabilities": [constants::SWAP_CAPABILITY]
        })
        .to_string(),
    }
}

pub fn escrow_descriptor_template(escrow_pk: &str, expires_at: u64) -> UnsignedEvent {
    UnsignedEvent {
        pubkey: escrow_pk.to_string(),
        created_at: crate::event::now(),
        kind: constants::KIND_ESCROW_DESCRIPTOR,
        tags: vec![
            vec!["d".to_string(), constants::ESCROW_D_TAG.to_string()],
            vec![
                "t".to_string(),
                format!("pontmore-network:{}", constants::ESCROW_NETWORK),
            ],
        ],
        content: serde_json::json!({
            "version": 1,
            "escrow_type": constants::ESCROW_TYPE,
            "networks": [constants::ESCROW_NETWORK],
            "expires_at": expires_at
        })
        .to_string(),
    }
}

pub struct RootInput {
    pub signer_pubkey: String,
    pub agent: String,
    pub customer: String,
    pub escrow: String,
    pub resolver: String,
    pub descriptor_id: String,
    pub terms: SwapTerms,
    pub expires_at: u64,
    pub commitments: Option<HashMap<String, CommitmentObject>>,
}

pub fn root_template(input: &RootInput) -> UnsignedEvent {
    let mut content = serde_json::json!({
        "version": constants::PIP02_VERSION,
        "profile": constants::SWAP_PROFILE,
        "terms": serde_json::to_value(&input.terms).unwrap(),
        "expires_at": input.expires_at,
    });
    if let Some(commitments) = &input.commitments {
        if !commitments.is_empty() {
            let c_map: HashMap<&str, &CommitmentObject> =
                commitments.iter().map(|(k, v)| (k.as_str(), v)).collect();
            content.as_object_mut().unwrap().insert(
                "commitments".to_string(),
                serde_json::to_value(&c_map).unwrap(),
            );
        }
    }
    UnsignedEvent {
        pubkey: input.signer_pubkey.clone(),
        created_at: crate::event::now(),
        kind: constants::KIND_ROOT,
        tags: vec![
            vec!["p".to_string(), input.agent.clone(), "".to_string(), "swap/agent".to_string()],
            vec![
                "p".to_string(),
                input.customer.clone(),
                "".to_string(),
                "swap/customer".to_string(),
            ],
            vec![
                "p".to_string(),
                input.escrow.clone(),
                "".to_string(),
                "core/escrow".to_string(),
            ],
            vec![
                "p".to_string(),
                input.resolver.clone(),
                "".to_string(),
                "core/resolver".to_string(),
            ],
            vec![
                "e".to_string(),
                input.descriptor_id.clone(),
                "".to_string(),
                "escrow-version".to_string(),
            ],
            vec![
                "a".to_string(),
                constants::escrow_address(&input.escrow),
                "".to_string(),
                "escrow".to_string(),
            ],
        ],
        content: content.to_string(),
    }
}

pub fn action_template(
    signer_pubkey: &str,
    root_id: &str,
    prev_id: &str,
    action: &str,
    data: Option<serde_json::Value>,
    routing: Vec<(String, String)>,
) -> UnsignedEvent {
    let mut content = serde_json::json!({
        "version": constants::PIP02_VERSION,
        "action": action,
    });
    if let Some(d) = data {
        content.as_object_mut().unwrap().insert("data".to_string(), d);
    }
    let mut tags = vec![
        vec!["e".to_string(), root_id.to_string(), "".to_string(), "root".to_string()],
        vec!["e".to_string(), prev_id.to_string(), "".to_string(), "prev".to_string()],
    ];
    for (pk, role) in &routing {
        tags.push(vec![
            "p".to_string(),
            pk.clone(),
            "".to_string(),
            role.clone(),
        ]);
    }
    UnsignedEvent {
        pubkey: signer_pubkey.to_string(),
        created_at: crate::event::now(),
        kind: constants::KIND_ACTION,
        tags,
        content: content.to_string(),
    }
}

pub fn sign_event(
    template: UnsignedEvent,
    secret_key: &[u8; 32],
) -> Result<NostrEvent, String> {
    template.sign(secret_key)
}

pub struct OfferInput {
    pub signer_pubkey: String,
    pub direction: crate::swap_v1::Direction,
    pub currency: String,
    pub price: String,
    pub min: String,
    pub max: String,
    pub channels: Vec<String>,
    pub channel_labels: Vec<String>,
    pub network: String,
    pub layer: String,
    pub premium: String,
    pub escrow: String,
    pub resolver: String,
    pub expires_at: u64,
    pub status: String,
    pub name: Option<String>,
    pub source: Option<String>,
}

pub fn offer_template(input: &OfferInput) -> UnsignedEvent {
    let order_type = if input.direction == crate::swap_v1::Direction::FiatToBtc {
        "sell"
    } else {
        "buy"
    };
    let d_tag = format!("{}:{}:{}", SWAP_PROFILE, input.currency, order_type);
    let live = input.status == "pending";
    let expiration = if live {
        input.expires_at.to_string()
    } else {
        (crate::event::now() + 3600).to_string()
    };
    let mut tags = vec![
        vec!["d".to_string(), d_tag],
        vec!["k".to_string(), order_type.to_string()],
        vec!["f".to_string(), input.currency.clone()],
        vec!["s".to_string(), input.status.clone()],
        vec!["amt".to_string(), "0".to_string()],
        vec!["fa".to_string(), input.min.clone(), input.max.clone()],
        {
            let mut t = vec!["pm".to_string()];
            t.extend(input.channel_labels.iter().cloned());
            t
        },
        vec!["premium".to_string(), input.premium.clone()],
        vec!["network".to_string(), input.network.clone()],
        vec!["layer".to_string(), input.layer.clone()],
        vec!["bond".to_string(), "0".to_string()],
        vec!["expires_at".to_string(), input.expires_at.to_string()],
        vec!["expiration".to_string(), expiration],
        vec!["y".to_string(), OFFER_PLATFORM.to_string()],
        vec!["z".to_string(), "order".to_string()],
        vec!["protocol".to_string(), SWAP_PROFILE.to_string()],
        vec!["price".to_string(), input.price.clone()],
        {
            let mut t = vec!["channels".to_string()];
            t.extend(input.channels.iter().cloned());
            t
        },
        vec!["a".to_string(), input.escrow.clone(), "".to_string(), "escrow".to_string()],
        vec!["p".to_string(), input.resolver.clone(), "".to_string(), "core/resolver".to_string()],
    ];
    if let Some(name) = &input.name {
        tags.push(vec!["name".to_string(), name.clone()]);
    }
    if let Some(source) = &input.source {
        tags.push(vec!["source".to_string(), source.clone()]);
    }
    UnsignedEvent {
        pubkey: input.signer_pubkey.clone(),
        created_at: crate::event::now(),
        kind: constants::KIND_OFFER,
        tags,
        content: String::new(),
    }
}