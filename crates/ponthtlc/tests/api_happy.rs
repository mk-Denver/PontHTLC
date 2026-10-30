use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bitcoin::hashes::Hash;
use lightning::events::{Event, PaymentPurpose};
use ponthtlc::bitcoind::BitcoindFacade;
use ponthtlc::http::{router, AppState};
use ponthtlc::node::{build_node, EventSink};
use ponthtlc::tenancy::EscrowStore;

fn curl_json(method: &str, url: &str, owner: &str, body: &str) -> (u16, serde_json::Value) {
    let header = format!("x-nostr-pubkey: {owner}");
    let mut command = std::process::Command::new("curl");
    command.args([
        "-sS",
        "-X",
        method,
        "-H",
        "content-type: application/json",
        "-H",
        &header,
        "-w",
        "\n%{http_code}",
    ]);
    if !body.is_empty() {
        command.args(["-d", body]);
    }
    command.arg(url);
    let output = command.output().expect("curl not found");
    let text = String::from_utf8(output.stdout).expect("curl output");
    let (payload, code) = text.rsplit_once('\n').unwrap_or((text.as_str(), "0"));
    let status = code.trim().parse().unwrap_or(0);
    let value = if payload.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(payload).unwrap_or(serde_json::Value::String(payload.to_string()))
    };
    (status, value)
}

async fn wait_for_event<F>(sink: &EventSink, mut predicate: F, timeout_secs: u64) -> Option<Event>
where
    F: FnMut(&Event) -> bool,
{
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        {
            let sink_lock = sink.lock().expect("event sink mutex poisoned");
            for event in sink_lock.iter() {
                if predicate(event) {
                    return Some(event.clone());
                }
            }
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn pubkey(seed: u8) -> String {
    hex::encode([seed; 32])
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_happy_path_pays_and_claims_without_storing_the_preimage() {
    let bitcoind = Arc::new(BitcoindFacade::new(
        "127.0.0.1".to_string(),
        18923,
        "ponthtlc".to_string(),
        "ponthtlc".to_string(),
    ));
    bitcoind.wait_for_bitcoind(45).await.expect("bitcoind");
    bitcoind.ensure_wallet("spike").await.expect("wallet");
    if bitcoind.get_block_height().await.expect("height") < 105 {
        bitcoind.generate_blocks(160).await.expect("mine");
    }

    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let root = std::env::temp_dir().join(format!("ponthtlc-api-happy-{nanos}"));
    let seller_dir = root.join("seller");
    let buyer_dir = root.join("buyer");
    std::fs::create_dir_all(&seller_dir).unwrap();
    std::fs::create_dir_all(&buyer_dir).unwrap();

    let seller = build_node("seller", seller_dir.clone(), 19820, Arc::clone(&bitcoind)).await;
    let buyer = build_node("buyer", buyer_dir.clone(), 19821, Arc::clone(&bitcoind)).await;
    seller.remember_peer(buyer.node_pubkey, buyer.listen_addr).await.expect("connect");
    for _ in 0..100 {
        let connected = seller.peer_manager.peer_by_node_id(&buyer.node_pubkey).is_some()
            && buyer.peer_manager.peer_by_node_id(&seller.node_pubkey).is_some();
        if connected {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        seller.peer_manager.peer_by_node_id(&buyer.node_pubkey).is_some()
            && buyer.peer_manager.peer_by_node_id(&seller.node_pubkey).is_some(),
        "peers did not connect"
    );

    seller
        .channel_manager
        .create_channel(buyer.node_pubkey, 1_000_000, 0, 0, None, None)
        .expect("create_channel");
    wait_for_event(&seller.events, |event| matches!(event, Event::FundingGenerationReady { .. }), 30)
        .await
        .expect("funding");
    tokio::time::sleep(Duration::from_millis(500)).await;
    bitcoind.generate_blocks(12).await.expect("confirm");
    wait_for_event(&seller.events, |event| matches!(event, Event::ChannelReady { .. }), 60)
        .await
        .expect("seller channel ready");
    wait_for_event(&buyer.events, |event| matches!(event, Event::ChannelReady { .. }), 60)
        .await
        .expect("buyer channel ready");
    tokio::time::sleep(Duration::from_secs(2)).await;

    let buyer_app = router(Arc::new(AppState {
        node: Arc::clone(&buyer),
        store: Mutex::new(EscrowStore::open(buyer_dir.join("escrows.json")).unwrap()),
        nostr_secret: [1u8; 32],
    }));
    let seller_app = router(Arc::new(AppState {
        node: Arc::clone(&seller),
        store: Mutex::new(EscrowStore::open(seller_dir.join("escrows.json")).unwrap()),
        nostr_secret: [2u8; 32],
    }));
    let buyer_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let seller_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let buyer_url = format!("http://{}", buyer_listener.local_addr().unwrap());
    let seller_url = format!("http://{}", seller_listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(buyer_listener, buyer_app).await.unwrap() });
    tokio::spawn(async move { axum::serve(seller_listener, seller_app).await.unwrap() });

    let mut preimage = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut preimage);
    let payment_hash = bitcoin::hashes::sha256::Hash::hash(&preimage).to_byte_array();
    let buyer_owner = pubkey(0x11);
    let seller_owner = pubkey(0x22);
    let (status, created) = curl_json(
        "POST",
        &format!("{buyer_url}/api/escrow/create"),
        &buyer_owner,
        &format!(
            "{{\"payment_hash\":\"{}\",\"amount_msat\":50000000}}",
            hex::encode(payment_hash)
        ),
    );
    let invoice = created["bolt11_invoice"].as_str().unwrap().to_string();
    let id = created["id"].as_str().unwrap().to_string();
    assert!(!invoice.is_empty());
    assert_eq!(status, 200, "{created}");

    let (status, paid) = curl_json(
        "POST",
        &format!("{seller_url}/api/escrow/{id}/fund"),
        &seller_owner,
        &format!("{{\"bolt11\":\"{invoice}\"}}"),
    );
    assert_eq!(status, 200, "{paid}");

    let claimable = wait_for_event(
        &buyer.events,
        |event| matches!(event, Event::PaymentClaimable { .. }),
        30,
    )
    .await
    .expect("buyer never received PaymentClaimable");
    match &claimable {
        Event::PaymentClaimable { purpose, .. } => match purpose {
            PaymentPurpose::Bolt11InvoicePayment { payment_preimage, .. } => {
                assert!(payment_preimage.is_none());
            },
            other => panic!("unexpected purpose: {other:?}"),
        },
        other => panic!("unexpected event: {other:?}"),
    }

    let (status, claimed) = curl_json(
        "POST",
        &format!("{buyer_url}/api/escrow/{id}/claim"),
        &buyer_owner,
        &format!("{{\"preimage\":\"{}\"}}", hex::encode(preimage)),
    );
    assert_eq!(status, 200, "{claimed}");

    wait_for_event(&seller.events, |event| matches!(event, Event::PaymentSent { .. }), 30)
        .await
        .expect("seller never saw PaymentSent");

    let (status, settled) = curl_json(
        "GET",
        &format!("{buyer_url}/api/escrow/{id}"),
        &buyer_owner,
        "",
    );
    assert_eq!(status, 200, "{settled}");
    assert_eq!(settled["protocol_status"].as_str().unwrap(), "settled");

    let mut fail_preimage = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut fail_preimage);
    let fail_hash = bitcoin::hashes::sha256::Hash::hash(&fail_preimage).to_byte_array();
    let (status, failed_invoice) = curl_json(
        "POST",
        &format!("{buyer_url}/api/escrow/create"),
        &buyer_owner,
        &format!(
            "{{\"payment_hash\":\"{}\",\"amount_msat\":10000000}}",
            hex::encode(fail_hash)
        ),
    );
    assert_eq!(status, 200, "{failed_invoice}");
    let fail_bolt11 = failed_invoice["bolt11_invoice"].as_str().unwrap();
    let fail_id = failed_invoice["id"].as_str().unwrap();
    let (status, funded_fail) = curl_json(
        "POST",
        &format!("{seller_url}/api/escrow/{fail_id}/fund"),
        &seller_owner,
        &format!("{{\"bolt11\":\"{fail_bolt11}\"}}"),
    );
    assert_eq!(status, 200, "{funded_fail}");
    wait_for_event(
        &buyer.events,
        |event| match event {
            Event::PaymentClaimable { payment_hash, .. } => payment_hash.0 == fail_hash,
            _ => false,
        },
        30,
    )
    .await
    .expect("second PaymentClaimable");
    let (status, failed) = curl_json(
        "POST",
        &format!("{buyer_url}/api/escrow/{fail_id}/fail"),
        &buyer_owner,
        "",
    );
    assert_eq!(status, 200, "{failed}");
    wait_for_event(&seller.events, |event| matches!(event, Event::PaymentFailed { .. }), 30)
        .await
        .expect("seller never saw PaymentFailed");

    assert_daemon_files_omit_preimage(&buyer_dir, &preimage);
    assert_daemon_files_omit_preimage(&seller_dir, &preimage);

    seller.shutdown().await;
    buyer.shutdown().await;
    let _ = std::fs::remove_dir_all(&root);
}

fn assert_daemon_files_omit_preimage(dir: &Path, preimage: &[u8; 32]) {
    let hex_secret = hex::encode(preimage);
    assert!(
        !daemon_files_contain_secret(dir, &hex_secret, preimage),
        "daemon file stored the preimage"
    );
}

fn daemon_files_contain_secret(dir: &Path, hex_secret: &str, raw_secret: &[u8]) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let child = entry.path();
        let name = child.file_name().and_then(|name| name.to_str()).unwrap_or("");
        // LDK writes the revealed preimage into the channel monitor and manager so it can settle.
        if name == "monitors" || name == "monitor_updates" || name == "manager" || name.starts_with("manager.") {
            continue;
        }
        if child.is_dir() {
            if daemon_files_contain_secret(&child, hex_secret, raw_secret) {
                return true;
            }
            continue;
        }
        let Ok(bytes) = std::fs::read(&child) else {
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
