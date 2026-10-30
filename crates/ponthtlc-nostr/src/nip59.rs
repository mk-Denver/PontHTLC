use ponthtlc_protocol::constants::KIND_GIFT_WRAP;
use ponthtlc_protocol::event::{is_hex64, NostrEvent, UnsignedEvent};

use crate::nip04;
use crate::payloads::{InboxItem, InboxKind, SwapPayload};

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
    let encrypted_to_recipient = nip04::encrypt(sender_sk, recipient_pk, rumor_json.as_bytes())?;
    let encrypted_to_sender = nip04::encrypt(sender_sk, sender_pk, rumor_json.as_bytes())?;

    let wrap_for_recipient = UnsignedEvent {
        pubkey: String::new(),
        created_at: ponthtlc_protocol::event::now(),
        kind: KIND_GIFT_WRAP,
        tags: vec![vec!["p".to_string(), recipient_pk.to_string()]],
        content: encrypted_to_recipient,
    };
    let wrap_for_sender = UnsignedEvent {
        pubkey: String::new(),
        created_at: ponthtlc_protocol::event::now(),
        kind: KIND_GIFT_WRAP,
        tags: vec![vec!["p".to_string(), sender_pk.to_string()]],
        content: encrypted_to_sender,
    };

    let (wrap_sk, wrap_pk) = ponthtlc_protocol::event::generate_keypair();
    let wrap1 = wrap_for_recipient.sign(&wrap_sk)?;
    let (wrap_sk2, _) = ponthtlc_protocol::event::generate_keypair();
    let wrap2 = wrap_for_sender.sign(&wrap_sk2)?;

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

    let encrypted_to_recipient = nip04::encrypt(sender_sk, recipient_pk, rumor_json.as_bytes())?;
    let encrypted_to_sender = nip04::encrypt(sender_sk, sender_pk, rumor_json.as_bytes())?;

    let (wrap_sk, _) = ponthtlc_protocol::event::generate_keypair();
    let (wrap_sk2, _) = ponthtlc_protocol::event::generate_keypair();

    let wrap1 = UnsignedEvent {
        pubkey: String::new(),
        created_at: ponthtlc_protocol::event::now(),
        kind: KIND_GIFT_WRAP,
        tags: vec![vec!["p".to_string(), recipient_pk.to_string()]],
        content: encrypted_to_recipient,
    }
    .sign(&wrap_sk)?;

    let wrap2 = UnsignedEvent {
        pubkey: String::new(),
        created_at: ponthtlc_protocol::event::now(),
        kind: KIND_GIFT_WRAP,
        tags: vec![vec!["p".to_string(), sender_pk.to_string()]],
        content: encrypted_to_sender,
    }
    .sign(&wrap_sk2)?;

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

    let decrypted = nip04::decrypt(recipient_sk, &wrap.pubkey, &wrap.content).ok()?;
    let rumor: NostrEvent = serde_json::from_slice(&decrypted).ok()?;

    if !rumor.verify() {
        return None;
    }

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

use ponthtlc_protocol::constants::SWAP_PROFILE;