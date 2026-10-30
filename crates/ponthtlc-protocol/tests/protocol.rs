use ponthtlc_protocol::constants::*;
use ponthtlc_protocol::event::{generate_keypair, is_hex64, now, NostrEvent};
use ponthtlc_protocol::events::*;
use ponthtlc_protocol::offer::*;
use ponthtlc_protocol::swap_v1::*;
use bitcoin::hashes::Hash;

fn make_terms(direction: Direction, pay_by: u64, confirm_by: u64) -> SwapTerms {
    SwapTerms {
        direction,
        fiat: FiatTerms {
            currency: "KES".to_string(),
            amount: "1000".to_string(),
        },
        bitcoin: BitcoinTerms {
            amount: "60000".to_string(),
            unit: "sat".to_string(),
            network: "lightning".to_string(),
        },
        payment_channel: "mpesa_phone_ke_kes@2".to_string(),
        deadlines: DeadlineTerms {
            fiat_pay_by: pay_by,
            fiat_confirm_by: confirm_by,
        },
    }
}

struct TestKeys {
    agent: ([u8; 32], String),
    customer: ([u8; 32], String),
    escrow: ([u8; 32], String),
    resolver: ([u8; 32], String),
}

fn test_keys() -> TestKeys {
    TestKeys {
        agent: generate_keypair(),
        customer: generate_keypair(),
        escrow: generate_keypair(),
        resolver: generate_keypair(),
    }
}

fn make_root(keys: &TestKeys, terms: &SwapTerms, expires_at: u64) -> NostrEvent {
    let desc = escrow_descriptor_template(&keys.escrow.1, expires_at + 86400);
    let desc_signed = desc.sign(&keys.escrow.0).unwrap();
    let input = RootInput {
        signer_pubkey: keys.customer.1.clone(),
        agent: keys.agent.1.clone(),
        customer: keys.customer.1.clone(),
        escrow: keys.escrow.1.clone(),
        resolver: keys.resolver.1.clone(),
        descriptor_id: desc_signed.id.clone(),
        terms: terms.clone(),
        expires_at,
        commitments: None,
    };
    let template = root_template(&input);
    template.sign(&keys.customer.0).unwrap()
}

fn make_action(
    signer_sk: &[u8; 32],
    signer_pk: &str,
    root_id: &str,
    prev_id: &str,
    action: &str,
    data: Option<serde_json::Value>,
) -> NostrEvent {
    let template = action_template(signer_pk, root_id, prev_id, action, data, vec![]);
    template.sign(signer_sk).unwrap()
}

fn full_happy_chain(
    keys: &TestKeys,
    root: &NostrEvent,
    pay_by: u64,
    confirm_by: u64,
) -> Vec<NostrEvent> {
    let root_id = &root.id;
    let accept = make_action(
        &keys.agent.0,
        &keys.agent.1,
        root_id,
        root_id,
        "core/accept",
        None,
    );
    let secure = make_action(
        &keys.escrow.0,
        &keys.escrow.1,
        root_id,
        &accept.id,
        "core/secure",
        None,
    );
    let fiat_sent = make_action(
        if fiat_sender(&parse_root(root, true).unwrap()) == &keys.customer.1 {
            &keys.customer.0
        } else {
            &keys.agent.0
        },
        if fiat_sender(&parse_root(root, true).unwrap()) == &keys.customer.1 {
            &keys.customer.1
        } else {
            &keys.agent.1
        },
        root_id,
        &secure.id,
        "swap/fiat_sent",
        Some(serde_json::json!({"payment_reference": "ref-123"})),
    );
    let fiat_confirmed = make_action(
        if fiat_receiver(&parse_root(root, true).unwrap()) == &keys.customer.1 {
            &keys.customer.0
        } else {
            &keys.agent.0
        },
        if fiat_receiver(&parse_root(root, true).unwrap()) == &keys.customer.1 {
            &keys.customer.1
        } else {
            &keys.agent.1
        },
        root_id,
        &fiat_sent.id,
        "swap/fiat_confirmed",
        Some(serde_json::json!({"payment_reference": "ref-123"})),
    );
    let authorize_settle = make_action(
        if fiat_receiver(&parse_root(root, true).unwrap()) == &keys.customer.1 {
            &keys.customer.0
        } else {
            &keys.agent.0
        },
        if fiat_receiver(&parse_root(root, true).unwrap()) == &keys.customer.1 {
            &keys.customer.1
        } else {
            &keys.agent.1
        },
        root_id,
        &fiat_confirmed.id,
        "core/authorize_settlement",
        None,
    );
    let settle = make_action(
        &keys.escrow.0,
        &keys.escrow.1,
        root_id,
        &authorize_settle.id,
        "core/settle",
        None,
    );
    vec![accept, secure, fiat_sent, fiat_confirmed, authorize_settle, settle]
}

#[test]
fn pip00_agent_definition_validates() {
    let keys = test_keys();
    let template = agent_definition_template(&keys.escrow.1, "");
    let event = template.sign(&keys.escrow.0).unwrap();
    assert_eq!(event.kind, KIND_AGENT_DEFINITION);
    assert!(event.verify());
    let t_tags = event.tags_by_name("t");
    assert!(t_tags.iter().any(|t| t.iter().any(|v| v == &capability_tag(SWAP_CAPABILITY))));
    let a_tag = event.tag("a").unwrap();
    assert!(a_tag.iter().any(|t| t.contains("30361:")));
    let content: serde_json::Value = serde_json::from_str(&event.content).unwrap();
    assert_eq!(content["capabilities"][0], SWAP_CAPABILITY);
}

#[test]
fn pip01_descriptor_validates() {
    let keys = test_keys();
    let template = escrow_descriptor_template(&keys.escrow.1, 1780000000);
    let event = template.sign(&keys.escrow.0).unwrap();
    assert_eq!(event.kind, KIND_ESCROW_DESCRIPTOR);
    assert!(event.verify());
    let content: serde_json::Value = serde_json::from_str(&event.content).unwrap();
    assert_eq!(content["escrow_type"], ESCROW_TYPE);
    assert_eq!(content["networks"][0], ESCROW_NETWORK);
    assert!(content["expires_at"].as_u64().is_some());
}

#[test]
fn pip02_root_validates() {
    let keys = test_keys();
    let now = now();
    let terms = make_terms(Direction::FiatToBtc, now + 3600, now + 7200);
    let root = make_root(&keys, &terms, now + 900);
    let parsed = parse_root(&root, true).unwrap();
    assert_eq!(parsed.agent, keys.agent.1);
    assert_eq!(parsed.customer, keys.customer.1);
    assert_eq!(parsed.escrow, keys.escrow.1);
    assert_eq!(parsed.resolver, keys.resolver.1);
    assert!(is_hex64(&parsed.descriptor_id));
    assert!(parsed.descriptor_address.starts_with("30361:"));
    assert_eq!(parsed.terms.direction, Direction::FiatToBtc);
    assert_eq!(parsed.terms.fiat.currency, "KES");
    assert_eq!(parsed.terms.bitcoin.unit, "sat");
}

#[test]
fn pip02_action_links() {
    let keys = test_keys();
    let now = now();
    let terms = make_terms(Direction::FiatToBtc, now + 3600, now + 7200);
    let root = make_root(&keys, &terms, now + 900);
    let accept = make_action(
        &keys.agent.0,
        &keys.agent.1,
        &root.id,
        &root.id,
        "core/accept",
        None,
    );
    let root_refs: Vec<_> = accept.tags_by_name("e").into_iter().filter(|t| t.get(3) == Some(&"root".to_string())).collect();
    let prev_refs: Vec<_> = accept.tags_by_name("e").into_iter().filter(|t| t.get(3) == Some(&"prev".to_string())).collect();
    assert_eq!(root_refs.len(), 1);
    assert_eq!(prev_refs.len(), 1);
    assert_eq!(root_refs[0].get(1), Some(&root.id));
    assert_eq!(prev_refs[0].get(1), Some(&root.id));
}

#[test]
fn pip02_rejects_unauthorized_signer() {
    let keys = test_keys();
    let now = now();
    let terms = make_terms(Direction::FiatToBtc, now + 3600, now + 7200);
    let root = make_root(&keys, &terms, now + 900);
    let rogue = generate_keypair();
    let accept = make_action(
        &rogue.0,
        &rogue.1,
        &root.id,
        &root.id,
        "core/accept",
        None,
    );
    let parsed = parse_root(&root, true).unwrap();
    let state = reconstruct(&parsed, &[accept], true);
    assert!(state.chain.is_empty());
    assert!(state.rejected.iter().any(|r| r.reason.contains("non-proposing")));
}

#[test]
fn pip02_rejects_replay() {
    let keys = test_keys();
    let now = now();
    let terms = make_terms(Direction::FiatToBtc, now + 3600, now + 7200);
    let root = make_root(&keys, &terms, now + 900);
    let accept = make_action(
        &keys.agent.0,
        &keys.agent.1,
        &root.id,
        &root.id,
        "core/accept",
        None,
    );
    let parsed = parse_root(&root, true).unwrap();
    let state = reconstruct(&parsed, &[accept.clone(), accept.clone()], true);
    assert_eq!(state.chain.len(), 1);
}

#[test]
fn pip02_rejects_fork() {
    let keys = test_keys();
    let now = now();
    let terms = make_terms(Direction::FiatToBtc, now + 3600, now + 7200);
    let root = make_root(&keys, &terms, now + 900);
    let parsed = parse_root(&root, true).unwrap();
    let accept = make_action(
        &keys.agent.0,
        &keys.agent.1,
        &root.id,
        &root.id,
        "core/accept",
        None,
    );
    let accept2 = make_action(
        &keys.agent.0,
        &keys.agent.1,
        &root.id,
        &root.id,
        "core/decline",
        None,
    );
    let state = reconstruct(&parsed, &[accept, accept2], true);
    assert!(state.forked);
    assert_eq!(state.fork_ids.len(), 2);
}

#[test]
fn pip02_rejects_post_terminal() {
    let keys = test_keys();
    let now = now();
    let terms = make_terms(Direction::FiatToBtc, now + 3600, now + 7200);
    let root = make_root(&keys, &terms, now + 900);
    let parsed = parse_root(&root, true).unwrap();
    let chain = full_happy_chain(&keys, &root, now + 3600, now + 7200);
    let extra = make_action(
        &keys.escrow.0,
        &keys.escrow.1,
        &root.id,
        &chain.last().unwrap().id,
        "core/settle",
        None,
    );
    let state = reconstruct(&parsed, &[chain.as_slice(), &[extra]].concat(), true);
    assert_eq!(state.status, SwapStatus::Settled);
    assert!(state.rejected.iter().any(|r| r.reason.contains("terminal")));
}

fn make_action_at(
    signer_sk: &[u8; 32],
    signer_pk: &str,
    root_id: &str,
    prev_id: &str,
    action: &str,
    data: Option<serde_json::Value>,
    created_at: u64,
) -> NostrEvent {
    let mut template = action_template(signer_pk, root_id, prev_id, action, data, vec![]);
    template.created_at = created_at;
    template.sign(signer_sk).unwrap()
}

#[test]
fn pip02_settle_refund_mutually_exclusive() {
    let keys = test_keys();
    let now = crate::now();
    let pay_by = now + 3600;
    let confirm_by = now + 7200;
    let terms = make_terms(Direction::FiatToBtc, pay_by, confirm_by);
    let root = make_root(&keys, &terms, now + 900);
    let parsed = parse_root(&root, true).unwrap();
    let accept = make_action(
        &keys.agent.0,
        &keys.agent.1,
        &root.id,
        &root.id,
        "core/accept",
        None,
    );
    let secure = make_action(
        &keys.escrow.0,
        &keys.escrow.1,
        &root.id,
        &accept.id,
        "core/secure",
        None,
    );
    let authorize_refund = make_action_at(
        &keys.agent.0,
        &keys.agent.1,
        &root.id,
        &secure.id,
        "core/authorize_refund",
        None,
        pay_by + 100,
    );
    let refund = make_action_at(
        &keys.escrow.0,
        &keys.escrow.1,
        &root.id,
        &authorize_refund.id,
        "core/refund",
        None,
        pay_by + 200,
    );
    let extra_settle = make_action_at(
        &keys.escrow.0,
        &keys.escrow.1,
        &root.id,
        &refund.id,
        "core/settle",
        None,
        pay_by + 300,
    );
    let state = reconstruct(
        &parsed,
        &[accept, secure, authorize_refund, refund, extra_settle].as_slice(),
        true,
    );
    assert_eq!(state.status, SwapStatus::Refunded);
    assert!(state.rejected.iter().any(|r| r.reason.contains("terminal")));
}

#[test]
fn swap_v1_terms_validate() {
    let now = now();
    let valid = serde_json::json!({
        "direction": "fiat_to_btc",
        "fiat": {"currency": "KES", "amount": "1000"},
        "bitcoin": {"amount": "60000", "unit": "sat", "network": "lightning"},
        "payment_channel": "mpesa_phone_ke_kes@2",
        "deadlines": {"fiat_pay_by": now + 3600, "fiat_confirm_by": now + 7200}
    });
    let terms = validate_terms(&valid, now + 900).unwrap();
    assert_eq!(terms.direction, Direction::FiatToBtc);
    assert_eq!(terms.fiat.currency, "KES");

    let bad_order = serde_json::json!({
        "direction": "fiat_to_btc",
        "fiat": {"currency": "KES", "amount": "1000"},
        "bitcoin": {"amount": "60000", "unit": "sat", "network": "lightning"},
        "payment_channel": "mpesa_phone_ke_kes@2",
        "deadlines": {"fiat_pay_by": now + 7200, "fiat_confirm_by": now + 3600}
    });
    assert!(validate_terms(&bad_order, now + 900).is_err());
}

#[test]
fn swap_v1_state_reconstruction() {
    let keys = test_keys();
    let now = now();
    let terms = make_terms(Direction::FiatToBtc, now + 3600, now + 7200);
    let root = make_root(&keys, &terms, now + 900);
    let parsed = parse_root(&root, true).unwrap();
    let chain = full_happy_chain(&keys, &root, now + 3600, now + 7200);
    let state = reconstruct(&parsed, &chain, true);
    assert_eq!(state.status, SwapStatus::Settled);
    assert!(!state.forked);
    assert_eq!(state.chain.len(), 6);
    let statuses: Vec<&str> = vec!["core/accept", "core/secure", "swap/fiat_sent", "swap/fiat_confirmed", "core/authorize_settlement", "core/settle"].into_iter().collect();
    for (i, expected) in statuses.iter().enumerate() {
        assert_eq!(state.chain[i].action, *expected);
    }
}

#[test]
fn swap_v1_dispute_overlay() {
    let keys = test_keys();
    let now = now();
    let terms = make_terms(Direction::FiatToBtc, now + 3600, now + 7200);
    let root = make_root(&keys, &terms, now + 900);
    let parsed = parse_root(&root, true).unwrap();
    let accept = make_action(
        &keys.agent.0,
        &keys.agent.1,
        &root.id,
        &root.id,
        "core/accept",
        None,
    );
    let secure = make_action(
        &keys.escrow.0,
        &keys.escrow.1,
        &root.id,
        &accept.id,
        "core/secure",
        None,
    );
    let dispute = make_action(
        &keys.customer.0,
        &keys.customer.1,
        &root.id,
        &secure.id,
        "core/open_dispute",
        Some(serde_json::json!({"class": "fiat_not_received"})),
    );
    let resolve = make_action(
        &keys.resolver.0,
        &keys.resolver.1,
        &root.id,
        &dispute.id,
        "core/resolve_dispute",
        Some(serde_json::json!({"policy": "pontmore-mobile/agent-resolver@1", "effect": "resume"})),
    );
    let state = reconstruct(&parsed, &[accept, secure, dispute, resolve], true);
    assert!(!state.disputed);
    assert_eq!(state.status, SwapStatus::Secured);
    assert!(state.resolution.is_some());
    assert_eq!(state.resolution.as_ref().unwrap().effect, "resume");
}

fn make_offer(keys: &TestKeys, price: &str, expires_at: u64) -> NostrEvent {
    let escrow_addr = escrow_address(&keys.escrow.1);
    let input = OfferInput {
        signer_pubkey: keys.agent.1.clone(),
        direction: Direction::FiatToBtc,
        currency: "KES".to_string(),
        price: price.to_string(),
        min: "100".to_string(),
        max: "50000".to_string(),
        channels: vec!["mpesa_phone_ke_kes@2".to_string()],
        channel_labels: vec!["M-Pesa Phone".to_string()],
        network: "regtest".to_string(),
        layer: "lightning".to_string(),
        premium: "1".to_string(),
        escrow: escrow_addr,
        resolver: keys.resolver.1.clone(),
        expires_at,
        status: "pending".to_string(),
        name: Some("Test Agent".to_string()),
        source: None,
    };
    let template = offer_template(&input);
    template.sign(&keys.agent.0).unwrap()
}

#[test]
fn nip69_offer_validates() {
    let keys = test_keys();
    let ts = now();
    let offer_event = make_offer(&keys, "1500000", ts + 1800);
    let offer = parse_offer(&offer_event, true).unwrap();
    assert_eq!(offer.content.direction, Direction::FiatToBtc);
    assert_eq!(offer.content.currency, "KES");
    assert_eq!(offer.content.price, "1500000");
    assert_eq!(offer.content.channels, vec!["mpesa_phone_ke_kes@2"]);
    assert_eq!(offer.content.escrow, escrow_address(&keys.escrow.1));
    assert_eq!(offer.content.resolver, keys.resolver.1);
    assert_eq!(offer.agent, keys.agent.1);
    assert_eq!(offer.content.status, OfferStatus::Pending);
    assert!(is_offer_live(&offer.content, ts));
    assert!(!is_offer_live(&offer.content, ts + 9999999));
}

#[test]
fn quote_commitment_matches() {
    let keys = test_keys();
    let ts = now();
    let offer_event = make_offer(&keys, "1500000", ts + 1800);
    let offer = parse_offer(&offer_event, true).unwrap();
    let q = quote(&offer.content, &Direction::FiatToBtc, "5000").unwrap();
    assert!(q.sats > 0);
    let computed = format!(
        "sha256:{}",
        hex::encode(bitcoin::hashes::sha256::Hash::hash(offer.raw.as_bytes()).to_byte_array())
    );
    let mut commitments = std::collections::HashMap::new();
    commitments.insert(
        "quote".to_string(),
        CommitmentObject {
            algorithm: "sha256-bytes@1".to_string(),
            digest: computed,
        },
    );
    let terms = terms_from_offer(
        &offer.content,
        &Direction::FiatToBtc,
        "5000",
        "mpesa_phone_ke_kes@2",
        ts + 3600,
        ts + 7200,
    )
    .unwrap();
    let desc = escrow_descriptor_template(&keys.escrow.1, ts + 86400);
    let desc_signed = desc.sign(&keys.escrow.0).unwrap();
    let input = RootInput {
        signer_pubkey: keys.customer.1.clone(),
        agent: keys.agent.1.clone(),
        customer: keys.customer.1.clone(),
        escrow: keys.escrow.1.clone(),
        resolver: keys.resolver.1.clone(),
        descriptor_id: desc_signed.id.clone(),
        terms,
        expires_at: ts + 900,
        commitments: Some(commitments),
    };
    let template = root_template(&input);
    let root = template.sign(&keys.customer.0).unwrap();
    let parsed = parse_root(&root, true).unwrap();
    check_root_against_offer(&parsed, &offer).unwrap();
}