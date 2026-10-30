use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lightning::events::Event;
use ponthtlc::bitcoind::BitcoindFacade;
use ponthtlc::node::{build_node, EventSink, Node};

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

fn channel_ids(node: &Node) -> Vec<String> {
    let mut ids = node
        .channel_manager
        .list_channels()
        .into_iter()
        .map(|channel| channel.channel_id.to_string())
        .collect::<Vec<_>>();
    ids.sort();
    ids
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restart_restores_channel_ids() {
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
    let root = std::env::temp_dir().join(format!("ponthtlc-restart-{nanos}"));
    let seller_dir = root.join("seller");
    let buyer_dir = root.join("buyer");
    std::fs::create_dir_all(&seller_dir).unwrap();
    std::fs::create_dir_all(&buyer_dir).unwrap();

    let seller = build_node("seller", seller_dir.clone(), 19810, Arc::clone(&bitcoind)).await;
    let buyer = build_node("buyer", buyer_dir.clone(), 19811, Arc::clone(&bitcoind)).await;
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

    let seller_ids = channel_ids(&seller);
    let buyer_ids = channel_ids(&buyer);
    assert!(!seller_ids.is_empty(), "seller opened no channels");

    let seller_dir: PathBuf = seller_dir;
    let buyer_dir: PathBuf = buyer_dir;
    seller.shutdown().await;
    buyer.shutdown().await;
    drop(seller);
    drop(buyer);
    tokio::time::sleep(Duration::from_millis(300)).await;

    let seller = build_node("seller", seller_dir, 19810, Arc::clone(&bitcoind)).await;
    let buyer = build_node("buyer", buyer_dir, 19811, bitcoind).await;
    assert_eq!(channel_ids(&seller), seller_ids);
    assert_eq!(channel_ids(&buyer), buyer_ids);
    seller.shutdown().await;
    buyer.shutdown().await;
    let _ = std::fs::remove_dir_all(&root);
}
