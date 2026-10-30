use std::time::Duration;

use ponthtlc_protocol::event::generate_keypair;
use ponthtlc_protocol::events::{agent_definition_template, escrow_descriptor_template};
use ponthtlc_nostr::nip59::{open_wrap, wrap_payload};
use ponthtlc_nostr::payloads::PayloadBody;
use ponthtlc_nostr::relay::Relay;

const RELAY_URL: &str = "ws://127.0.0.1:7777";

async fn relay_available() -> bool {
    Relay::connect(RELAY_URL).await.is_ok()
}

#[tokio::test]
async fn relay_publish_and_recv_agent_definition() {
    if !relay_available().await {
        eprintln!("skipping: relay not available at {RELAY_URL}");
        return;
    }

    let (sk, pk) = generate_keypair();
    let (_escrow_sk, escrow_pk) = generate_keypair();

    let agent_template = agent_definition_template(&escrow_pk, "");
    let agent_event = agent_template.sign(&sk).unwrap();
    assert_eq!(agent_event.kind, 30360);
    assert!(agent_event.verify());
    let agent_id = agent_event.id.clone();

    let relay = Relay::connect(RELAY_URL).await.expect("connect relay");

    relay
        .subscribe("test-agent", &[30360], &[&pk], &[], &[], None, None, Some(10))
        .await
        .expect("subscribe");
    tokio::time::sleep(Duration::from_millis(200)).await;

    relay.publish(&agent_event).await.expect("publish");

    let mut received = None;
    for _ in 0..100 {
        match tokio::time::timeout(Duration::from_secs(1), relay.recv()).await {
            Ok(Ok(Some((_sub_id, event)))) => {
                if event.id == agent_id {
                    received = Some(event);
                    break;
                }
            }
            _ => continue,
        }
    }

    let received = received.expect("did not receive agent definition from relay");
    assert_eq!(received.kind, 30360);
    assert_eq!(received.pubkey, pk);
    assert!(received.verify());
}

#[tokio::test]
async fn nip59_gift_wrap_round_trip() {
    if !relay_available().await {
        eprintln!("skipping: relay not available at {RELAY_URL}");
        return;
    }

    let (sender_sk, sender_pk) = generate_keypair();
    let (recipient_sk, recipient_pk) = generate_keypair();
    let coordination_id = "a".repeat(64);

    let preimage_hex = "ee".repeat(32);
    let body = PayloadBody::Release {
        preimage: preimage_hex.clone(),
    };

    let wraps = wrap_payload(
        &sender_sk,
        &sender_pk,
        &recipient_pk,
        &coordination_id,
        [sender_pk.clone(), recipient_pk.clone()],
        body,
    )
    .expect("wrap payload");

    assert_eq!(wraps.len(), 2);
    let recipient_wrap = &wraps[0];
    assert_eq!(recipient_wrap.kind, 1059);
    assert!(recipient_wrap.verify());
    let wrap_id = recipient_wrap.id.clone();

    let relay = Relay::connect(RELAY_URL).await.expect("connect relay");

    relay
        .subscribe("test-nip59", &[1059], &[], &[&recipient_pk], &[], None, None, Some(10))
        .await
        .expect("subscribe");
    tokio::time::sleep(Duration::from_millis(200)).await;

    relay.publish(recipient_wrap).await.expect("publish gift wrap");

    let mut received_wrap = None;
    for _ in 0..100 {
        match tokio::time::timeout(Duration::from_secs(1), relay.recv()).await {
            Ok(Ok(Some((_sub_id, event)))) => {
                if event.id == wrap_id {
                    received_wrap = Some(event);
                    break;
                }
            }
            _ => continue,
        }
    }

    let received_wrap = received_wrap.expect("did not receive gift wrap from relay");
    assert_eq!(received_wrap.id, wrap_id);

    let inbox = open_wrap(&received_wrap, &recipient_sk).expect("open_wrap failed");
    assert_eq!(inbox.from, sender_pk);
    assert_eq!(inbox.to, recipient_pk);
    assert_eq!(inbox.coordination, coordination_id);

    let payload = inbox.payload.expect("payload missing");
    assert_eq!(payload.profile, "pontmore/swap@1");
    assert_eq!(payload.coordination, coordination_id);
    assert_eq!(payload.participants, vec![sender_pk, recipient_pk]);

    match &payload.body {
        PayloadBody::Release { preimage } => {
            assert_eq!(*preimage, preimage_hex);
        }
        _ => panic!("wrong payload body type"),
    }
}