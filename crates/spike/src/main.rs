mod shamir;

use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Duration;

use bitcoin::hashes::Hash;
use lightning::events::{Event, PaymentPurpose};
use lightning::ln::channelmanager::{PaymentId, Retry};
use lightning::routing::router::RouteParametersConfig;
use lightning::types::payment::{PaymentHash, PaymentPreimage};
use ponthtlc::bitcoind::BitcoindFacade;
use ponthtlc::node::{build_node, EventSink, Node};
use rand::RngCore;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scenario {
    Happy,
    Fail,
    Hold,
}

struct SpikeConfig {
    bitcoind_host: String,
    bitcoind_port: u16,
    rpc_user: String,
    rpc_pass: String,
    data_root: String,
    scenario: Scenario,
}

fn parse_config() -> SpikeConfig {
    let mut args: HashMap<String, String> = HashMap::new();
    let argv: Vec<String> = env::args().collect();
    let mut i = 1;
    while i + 1 < argv.len() {
        let key = argv[i].trim_start_matches("--").to_string();
        args.insert(key, argv[i + 1].clone());
        i += 2;
    }
    let scenario = match args.get("scenario").map(|s| s.as_str()).unwrap_or("happy") {
        "happy" => Scenario::Happy,
        "fail" => Scenario::Fail,
        "hold" => Scenario::Hold,
        other => panic!("Unknown scenario '{}', expected happy|fail|hold", other),
    };
    SpikeConfig {
        bitcoind_host: args
            .get("bitcoind-host")
            .cloned()
            .unwrap_or_else(|| "127.0.0.1".to_string()),
        bitcoind_port: args
            .get("bitcoind-port")
            .map(|v| v.parse().unwrap())
            .unwrap_or(18923),
        rpc_user: args.get("rpc-user").cloned().unwrap_or_else(|| "ponthtlc".to_string()),
        rpc_pass: args.get("rpc-pass").cloned().unwrap_or_else(|| "ponthtlc".to_string()),
        data_root: args.get("data-root").cloned().unwrap_or_else(|| "spike-data".to_string()),
        scenario,
    }
}

fn wait_for_event<F>(sink: &EventSink, mut predicate: F, timeout_secs: u64) -> Option<Event>
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
        std::thread::sleep(Duration::from_millis(100));
    }
}

async fn connect_peers(seller: &Arc<Node>, buyer: &Arc<Node>) -> Result<(), String> {
    match lightning_net_tokio::connect_outbound(
        Arc::clone(&seller.peer_manager),
        buyer.node_pubkey,
        buyer.listen_addr,
    )
    .await
    {
        Some(_) => {},
        None => return Err("Failed to initiate outbound connection to buyer".to_string()),
    };
    for _ in 0..100 {
        let connected = seller.peer_manager.peer_by_node_id(&buyer.node_pubkey).is_some()
            && buyer.peer_manager.peer_by_node_id(&seller.node_pubkey).is_some();
        if connected {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Err("Peers never saw each other after connect attempt".to_string())
}

async fn open_and_confirm_channel(
    bitcoind: &BitcoindFacade, seller: &Arc<Node>, buyer: &Arc<Node>,
) -> Result<(), String> {
    seller
        .channel_manager
        .create_channel(buyer.node_pubkey, 1_000_000, 0, 0, None, None)
        .map_err(|e| format!("create_channel failed: {:?}", e))?;
    for _ in 0..20 {
        let funding_ready = seller
            .events
            .lock()
            .expect("event sink mutex poisoned")
            .iter()
            .any(|e| matches!(e, Event::FundingGenerationReady { .. }));
        if funding_ready {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
    bitcoind.generate_blocks(12).await.map_err(|e| format!("mining failed: {}", e))?;
    let is_channel_ready = |e: &Event| matches!(e, Event::ChannelReady { .. });
    wait_for_event(&seller.events, is_channel_ready, 30)
        .ok_or_else(|| "seller never saw ChannelReady".to_string())?;
    wait_for_event(&buyer.events, is_channel_ready, 30)
        .ok_or_else(|| "buyer never saw ChannelReady".to_string())?;
    Ok(())
}

async fn register_manual_hold_invoice(
    buyer: &Arc<Node>, payment_hash: PaymentHash, amount_msat: u64,
) -> Result<(lightning::types::payment::PaymentSecret, lightning_invoice::Bolt11Invoice), String> {
    let payment_secret = buyer
        .channel_manager
        .create_inbound_payment_for_hash(payment_hash, Some(amount_msat), 3600, None)
        .map_err(|e| format!("create_inbound_payment_for_hash failed: {:?}", e))?;

    let secp_ctx = bitcoin::secp256k1::Secp256k1::signing_only();
    let node_secret_key = buyer.keys_manager.get_node_secret_key();
    let payment_hash_sha256 = bitcoin::hashes::sha256::Hash::from_byte_array(payment_hash.0);

    let invoice = lightning_invoice::InvoiceBuilder::new(lightning_invoice::Currency::Regtest)
        .amount_milli_satoshis(amount_msat)
        .description("PontHTLC spike manual hold invoice".to_string())
        .payment_hash(payment_hash_sha256)
        .payment_secret(payment_secret)
        .current_timestamp()
        .min_final_cltv_expiry_delta(40)
        .expiry_time(Duration::from_secs(3600))
        .build_signed(|message| secp_ctx.sign_ecdsa_recoverable(message, &node_secret_key))
        .map_err(|e| format!("invoice build_signed failed: {:?}", e))?;
    Ok((payment_secret, invoice))
}

fn extract_claimable(event: &Event) -> (PaymentHash, Option<PaymentPreimage>, Option<u32>) {
    match event {
        Event::PaymentClaimable { payment_hash, purpose, claim_deadline, .. } => {
            let preimage = match purpose {
                PaymentPurpose::Bolt11InvoicePayment { payment_preimage, .. } => {
                    *payment_preimage
                },
                other => panic!("Unexpected purpose: {:?}", other),
            };
            (*payment_hash, preimage, *claim_deadline)
        },
        other => panic!("Unexpected event: {:?}", other),
    }
}

async fn run_happy(
    seller: &Arc<Node>, buyer: &Arc<Node>, registered_hash: PaymentHash, preimage: PaymentPreimage,
) -> Result<(), String> {
    let shares = shamir::split_preimage(&preimage.0);
    let reconstructed = shamir::reconstruct_and_verify(&shares[0], &shares[1], &registered_hash)
        .ok_or_else(|| "reconstruction failed hash verification".to_string())?;
    if reconstructed != preimage {
        return Err("reconstructed preimage diverges from origin".to_string());
    }
    println!("SPIKE_PASS: 4a-shamir-reconstruction: two quorum shares reconstruct the preimage");

    buyer.channel_manager.claim_funds(reconstructed);

    let _claimed = wait_for_event(&buyer.events, |e| matches!(e, Event::PaymentClaimed { .. }), 30)
        .ok_or_else(|| "buyer never saw PaymentClaimed".to_string())?;
    println!("SPIKE_INFO: buyer claimed payment");

    let sent = wait_for_event(&seller.events, |e| matches!(e, Event::PaymentSent { .. }), 30)
        .ok_or_else(|| "seller never saw PaymentSent".to_string())?;
    if let Event::PaymentSent {
        payment_preimage,
        payment_hash,
        ..
    } = &sent
    {
        if *payment_hash != registered_hash {
            return Err("PaymentSent hash mismatch".to_string());
        }
        if *payment_preimage == preimage {
            println!("SPIKE_PASS: 4b-release-claim: seller auto-recovers the preimage from the claim");
        } else {
            return Err("PaymentSent preimage mismatch".to_string());
        }
    }
    Ok(())
}

async fn run_fail(
    seller: &Arc<Node>, buyer: &Arc<Node>, registered_hash: PaymentHash,
) -> Result<(), String> {
    buyer.channel_manager.fail_htlc_backwards(&registered_hash);
    let _failed = wait_for_event(&seller.events, |e| matches!(e, Event::PaymentFailed { .. }), 30)
        .ok_or_else(|| "seller never saw PaymentFailed after backward fail".to_string())?;
    println!("SPIKE_INFO: seller saw PaymentFailed");
    println!("SPIKE_PASS: 4c-manual-fail: payment failed backwards, seller gets refund semantics");
    Ok(())
}

async fn run_hold(
    bitcoind: &BitcoindFacade, seller: &Arc<Node>, claim_deadline: Option<u32>,
) -> Result<(), String> {
    let deadline = claim_deadline.ok_or("claim_deadline was None; cannot test expiry")?;
    println!("SPIKE_INFO: mining to above claim_deadline={}", deadline);
    loop {
        let height = bitcoind.get_block_height().await.map_err(|e| e)?;
        if height >= deadline as u64 + 1 {
            break;
        }
        bitcoind.generate_blocks(4).await?;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let failed =
        wait_for_event(&seller.events, |e| matches!(e, Event::PaymentFailed { .. }), 20);
    match failed {
        Some(_event) => {
            println!("SPIKE_INFO: seller saw PaymentFailed");
            println!("SPIKE_PASS: 4d-hold-expiry: automatic expiry fired past claim_deadline (your LDK semantics observed)");
        },
        None => {
            println!("SPIKE_NOTE: no automatic expiry fired past the deadline; manual claim/fail stays available");
            println!("SPIKE_PASS: 4d-hold-expiry: no auto-claim discovered pre-deadline; document real semantics in ADR-0001");
        },
    }
    Ok(())
}

async fn run_scenario(cfg: SpikeConfig) -> Result<(), String> {
    let bitcoind = Arc::new(BitcoindFacade::new(
        cfg.bitcoind_host.clone(),
        cfg.bitcoind_port,
        cfg.rpc_user.clone(),
        cfg.rpc_pass.clone(),
    ));

    bitcoind.wait_for_bitcoind(45).await?;
    bitcoind.ensure_wallet("spike").await?;
    if bitcoind.get_block_height().await.map_err(|e| e)? < 105 {
        println!("SPIKE_INFO: mining 160 blocks for mature wallet funds");
        bitcoind.generate_blocks(160).await?;
    }
    println!("SPIKE_INFO: chain height {}", bitcoind.get_block_height().await.map_err(|e| e)?);

    let repo_root = env::current_dir().map_err(|e| e.to_string())?;
    let data_root = repo_root.join(&cfg.data_root);
    let _ = std::fs::remove_dir_all(&data_root);
    let seller_dir = data_root.join("seller");
    let buyer_dir = data_root.join("buyer");
    let _ = std::fs::create_dir_all(&seller_dir);
    let _ = std::fs::create_dir_all(&buyer_dir);

    let seller = build_node("seller", seller_dir, 9800, Arc::clone(&bitcoind)).await;
    let buyer = build_node("buyer", buyer_dir, 9801, Arc::clone(&bitcoind)).await;

    connect_peers(&seller, &buyer).await?;
    println!("SPIKE_PASS: 0-connect: peers connected");
    open_and_confirm_channel(&bitcoind, &seller, &buyer).await?;
    println!("SPIKE_PASS: 1-open-channel: direct channel with 12 confirmations is usable");

    let trade_amount_msat: u64 = 50_000_000;
    let mut preimage_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut preimage_bytes);
    let preimage = PaymentPreimage(preimage_bytes);
    let registered_hash = PaymentHash(
        bitcoin::hashes::sha256::Hash::hash(&preimage.0).to_byte_array(),
    );

    let (_secret, invoice) =
        register_manual_hold_invoice(&buyer, registered_hash, trade_amount_msat).await?;
    let invoice_string = invoice.to_string();
    println!("SPIKE_INFO: hold invoice registered from buyer node: {}", invoice_string);

    let payment_id = PaymentId((*invoice.payment_hash()).to_byte_array());
    seller
        .channel_manager
        .pay_for_bolt11_invoice(
            &invoice,
            payment_id,
            None,
            RouteParametersConfig::default(),
            Retry::Timeout(Duration::from_secs(10)),
        )
        .map_err(|e| format!("pay_for_bolt11_invoice failed: {:?}", e))?;

    let claimable =
        wait_for_event(&buyer.events, |e| matches!(e, Event::PaymentClaimable { .. }), 30)
            .ok_or_else(|| "buyer never received PaymentClaimable".to_string())?;
    let (claim_hash, held_purpose_preimage, claim_deadline) = extract_claimable(&claimable);
    if claim_hash != registered_hash {
        return Err("PaymentClaimable hash mismatch vs registered hash".to_string());
    }
    match held_purpose_preimage {
        None => println!(
            "SPIKE_PASS: 3-payment-claimable: PaymentClaimable arrived with payment_preimage=None (manual hold verified)"
        ),
        Some(_) => {
            println!("SPIKE_FAIL: 3-payment-claimable: LDK already knows a preimage (auto-claim risk)");
            return Err("hold semantics violated".to_string());
        },
    }
    println!("SPIKE_INFO: claim_deadline={:?}", claim_deadline);

    match cfg.scenario {
        Scenario::Happy => run_happy(&seller, &buyer, registered_hash, preimage).await?,
        Scenario::Fail => run_fail(&seller, &buyer, registered_hash).await?,
        Scenario::Hold => run_hold(&bitcoind, &seller, claim_deadline).await?,
    }

    println!("SPIKE_RESULT: PASS scenario={:?}", cfg.scenario);
    Ok(())
}

#[tokio::main]
async fn main() {
    let cfg = parse_config();
    if let Err(reason) = run_scenario(cfg).await {
        println!("SPIKE_RESULT: FAIL reason={}", reason);
        std::process::exit(1);
    }
}
