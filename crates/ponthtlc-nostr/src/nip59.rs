use ponthtlc_protocol::constants::{KIND_GIFT_WRAP, KIND_SEAL, SWAP_PROFILE};
use ponthtlc_protocol::event::{is_hex64, NostrEvent, UnsignedEvent};

use crate::nip04;
use crate::payloads::{InboxItem, InboxKind, SwapPayload};

fn build_seal(
    sender_sk: &[u8; 32],
    sender_pk: &str,
    recipient_pk: &str,
    rumor_json: &str,
) -> Result<NostrEvent, String> {
    let encrypted = nip04::encrypt(sender_sk, recipient_pk, rumor_json.as_bytes())?;
    let seal = UnsignedEvent {
        pubkey: sender_pk.to_string(),
        created_at: ponthtlc_protocol::event::now(),
        kind: KIND_SEAL,
        tags: vec![vec!["p".to_string(), recipient_pk.to_string()]],
        content: encrypted,
    };
    seal.sign(sender_sk)
}

fn open_seal(
    seal: &NostrEvent,
    recipient_sk: &[u8; 32],
) -> Option<NostrEvent> {
    if seal.kind != KIND_SEAL {
        return None;
    }
    if !seal.verify() {
        return None;
    }
    let decrypted = nip04::decrypt(recipient_sk, &seal.pubkey, &seal.content).ok()?;
    let rumor: NostrEvent = serde_json::from_slice(&decrypted).ok()?;
    if !rumor.verify() {
        return None;
    }
    Some(rumor)
}

fn wrap_seal(
    seal: &NostrEvent,
    recipient_pk: &str,
) -> Result<NostrEvent, String> {
    let seal_json = serde_json::to_string(seal).map_err(|e| format!("serialize seal: {e}"))?;
    let (wrap_sk, wrap_pk) = ponthtlc_protocol::event::generate_keypair();
    let encrypted = nip04::encrypt(&wrap_sk, recipient_pk, seal_json.as_bytes())?;
    let wrap = UnsignedEvent {
        pubkey: wrap_pk,
        created_at: ponthtlc_protocol::event::now(),
        kind: KIND_GIFT_WRAP,
        tags: vec![vec!["p".to_string(), recipient_pk.to_string()]],
        content: encrypted,
    };
    wrap.sign(&wrap_sk)
}

pub fn wrap_payload(
    sender_sk: &[u8; 32],
    sender_pk: &str,
    recipient_pk: &str,
    coordination: &str,
    participants: [String; 2],
    body: crate::payloads::PayloadBody,
) -> Result<Vec<NostrEvent>, String> {
    let payload = SwapPayload::new(coordination, participants, body);
    let payload_json = serde_json::to_string(&payload).map_err(|e| format!("serialize payload: {e}"))?;

    let rumor = UnsignedEvent {
        pubkey: sender_pk.to_string(),
        created_at: ponthtlc_protocol::event::now(),
        kind: ponthtlc_protocol::constants::KIND_SWAP_PAYLOAD,
        tags: vec![
            vec!["p".to_string(), recipient_pk.to_string()],
            vec!["e".to_string(), coordination.to_string(), "".to_string(), "root".to_string()],
        ],
        content: payload_json,
    };
    let signed_rumor = rumor.sign(sender_sk)?;
    let rumor_json = serde_json::to_string(&signed_rumor).map_err(|e| format!("serialize rumor: {e}"))?;

    let seal_to_recipient = build_seal(sender_sk, sender_pk, recipient_pk, &rumor_json)?;
    let seal_to_sender = build_seal(sender_sk, sender_pk, sender_pk, &rumor_json)?;

    let wrap1 = wrap_seal(&seal_to_recipient, recipient_pk)?;
    let wrap2 = wrap_seal(&seal_to_sender, sender_pk)?;

    Ok(vec![wrap1, wrap2])
}

pub fn wrap_chat(
    sender_sk: &[u8; 32],
    sender_pk: &str,
    recipient_pk: &str,
    coordination: &str,
    text: &str,
) -> Result<Vec<NostrEvent>, String> {
    let rumor = UnsignedEvent {
        pubkey: sender_pk.to_string(),
        created_at: ponthtlc_protocol::event::now(),
        kind: ponthtlc_protocol::constants::KIND_CHAT,
        tags: vec![
            vec!["p".to_string(), recipient_pk.to_string()],
            vec!["e".to_string(), coordination.to_string(), "".to_string(), "root".to_string()],
            vec!["subject".to_string(), "Pontmore swap".to_string()],
        ],
        content: text.chars().take(4000).collect(),
    };
    let signed_rumor = rumor.sign(sender_sk)?;
    let rumor_json = serde_json::to_string(&signed_rumor).map_err(|e| format!("serialize rumor: {e}"))?;

    let seal_to_recipient = build_seal(sender_sk, sender_pk, recipient_pk, &rumor_json)?;
    let seal_to_sender = build_seal(sender_sk, sender_pk, sender_pk, &rumor_json)?;

    let wrap1 = wrap_seal(&seal_to_recipient, recipient_pk)?;
    let wrap2 = wrap_seal(&seal_to_sender, sender_pk)?;

    Ok(vec![wrap1, wrap2])
}

pub fn open_wrap(wrap: &NostrEvent, recipient_sk: &[u8; 32]) -> Option<InboxItem> {
    if wrap.kind != KIND_GIFT_WRAP {
        return None;
    }
    if !wrap.verify() {
        return None;
    }
    let p_tags: Vec<&[String]> = wrap.tags_by_name("p").into_iter().collect();
    if p_tags.len() != 1 {
        return None;
    }
    let to = p_tags[0].get(1)?;
    if !is_hex64(to) {
        return None;
    }

    let our_pk = ponthtlc_protocol::event::pubkey_from_secret(recipient_sk);
    if to != &our_pk {
        return None;
    }

    let seal_decrypted = nip04::decrypt(recipient_sk, &wrap.pubkey, &wrap.content).ok()?;
    let seal: NostrEvent = serde_json::from_slice(&seal_decrypted).ok()?;

    let rumor = open_seal(&seal, recipient_sk)?;

    let e_tags: Vec<&[String]> = rumor.tags_by_name("e").into_iter().collect();
    if e_tags.len() != 1 {
        return None;
    }
    let coordination = e_tags[0].get(1)?.clone();

    let base = InboxItem {
        id: rumor.id.clone(),
        from: rumor.pubkey.clone(),
        to: to.clone(),
        created_at: rumor.created_at,
        coordination,
        kind: InboxKind::Chat,
        text: None,
        payload: None,
        raw_event: Some(rumor.clone()),
    };

    if rumor.kind == ponthtlc_protocol::constants::KIND_CHAT {
        return Some(InboxItem {
            text: Some(rumor.content.chars().take(4000).collect()),
            ..base
        });
    }

    if rumor.kind != ponthtlc_protocol::constants::KIND_SWAP_PAYLOAD {
        return None;
    }

    let payload: SwapPayload = serde_json::from_str(&rumor.content).ok()?;
    if payload.profile != SWAP_PROFILE {
        return None;
    }
    if payload.coordination != base.coordination {
        return None;
    }
    if payload.participants.len() != 2 {
        return None;
    }
    if !payload.participants.iter().all(|p| is_hex64(p)) {
        return None;
    }
    if !payload.participants.contains(&rumor.pubkey) || !payload.participants.contains(to) {
        return None;
    }
    if payload.commitment_scheme != "sha256-bytes@1" {
        return None;
    }

    Some(InboxItem {
        kind: InboxKind::Payload,
        payload: Some(payload),
        ..base
    })
}