use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderValue, Method, HeaderMap, StatusCode};
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::extract::Request;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use lightning::ln::channelmanager::{PaymentId, Retry};
use lightning::routing::router::RouteParametersConfig;
use lightning::types::payment::{PaymentHash, PaymentPreimage};
use ponthtlc_escrow::{EscrowNode, LdkBridge};
use ponthtlc_nostr::nip59::wrap_payload;
use ponthtlc_nostr::payloads::PayloadBody;
use ponthtlc_protocol::event::{pubkey_from_secret, NostrEvent};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::dual::DualNode;
use crate::events::WireEvent;
use crate::invoice::{self, payment_hash_bytes};
use crate::node::{ChannelSnapshot, Node};
use crate::tenancy::{EscrowRecord, EscrowStore};

const NOSTR_PUBKEY_HEADER: &str = "x-nostr-pubkey";
const ARBITRATOR_SHARE_TERMS: &str = "arbitrator-share";
const ARBITRATOR_SHARE_NAME: &str = "s3";

pub trait HoldNode: Send + Sync {
    fn register_hold(&self, payment_hash: [u8; 32], amount_msat: u64) -> Result<String, String>;
    fn claim(&self, preimage: [u8; 32], payment_hash: [u8; 32]) -> Result<(), String>;
    fn fail(&self, payment_hash: [u8; 32]) -> Result<(), String>;
    fn pay(&self, bolt11: &str) -> Result<[u8; 32], String>;
    fn subscribe(&self) -> broadcast::Receiver<WireEvent>;
    fn node_pubkey(&self) -> String;
    fn buyer_pubkey(&self) -> String {
        self.node_pubkey()
    }
    fn seller_pubkey(&self) -> String {
        self.node_pubkey()
    }
    fn channel_ids(&self) -> Vec<String>;
    fn channel_snapshots(&self) -> Vec<ChannelSnapshot>;
    fn peers_connected(&self) -> bool {
        true
    }
    fn open_channel(
        &self, capacity_satoshis: u64,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + '_>>;
    fn block_height(&self) -> Pin<Box<dyn Future<Output = Result<u64, String>> + Send>>;
}

struct HostedNode(Arc<Node>);

impl EscrowNode for HostedNode {
    fn register_hold(
        &self,
        payment_hash: PaymentHash,
        amount_msat: u64,
    ) -> Result<lightning::types::payment::PaymentSecret, String> {
        self.0
            .channel_manager
            .create_inbound_payment_for_hash(payment_hash, Some(amount_msat), 3600, None)
            .map_err(|err| format!("create_inbound_payment_for_hash failed: {:?}", err))
    }

    fn claim(&self, preimage: PaymentPreimage) -> Result<(), String> {
        self.0.channel_manager.claim_funds(preimage);
        Ok(())
    }

    fn fail_htlc(&self, payment_hash: PaymentHash) -> Result<(), String> {
        self.0.channel_manager.fail_htlc_backwards(&payment_hash);
        Ok(())
    }
}

impl HoldNode for Arc<Node> {
    fn register_hold(&self, payment_hash: [u8; 32], amount_msat: u64) -> Result<String, String> {
        let payment_secret =
            LdkBridge::new(HostedNode(Arc::clone(self))).secure(&PaymentHash(payment_hash), amount_msat)?;
        let invoice = invoice::build_hold_invoice(
            &self.keys_manager,
            payment_hash,
            payment_secret,
            amount_msat,
            lightning_invoice::Currency::Regtest,
        )?;
        Ok(invoice.to_string())
    }

    fn claim(&self, preimage: [u8; 32], payment_hash: [u8; 32]) -> Result<(), String> {
        LdkBridge::new(HostedNode(Arc::clone(self)))
            .settle(PaymentPreimage(preimage), &PaymentHash(payment_hash))
    }

    fn fail(&self, payment_hash: [u8; 32]) -> Result<(), String> {
        LdkBridge::new(HostedNode(Arc::clone(self))).refund(&PaymentHash(payment_hash))
    }

    fn pay(&self, bolt11: &str) -> Result<[u8; 32], String> {
        let invoice = invoice::parse_bolt11(bolt11)?;
        let payment_hash = payment_hash_bytes(&invoice);
        let payment_id = PaymentId(payment_hash);
        self.channel_manager
            .pay_for_bolt11_invoice(
                &invoice,
                payment_id,
                None,
                RouteParametersConfig::default(),
                Retry::Timeout(Duration::from_secs(10)),
            )
            .map_err(|err| format!("pay_for_bolt11_invoice failed: {:?}", err))?;
        Ok(payment_hash)
    }

    fn subscribe(&self) -> broadcast::Receiver<WireEvent> {
        Node::subscribe(self)
    }

    fn node_pubkey(&self) -> String {
        self.node_pubkey.to_string()
    }

    fn channel_ids(&self) -> Vec<String> {
        Node::listed_channel_ids(self)
    }

    fn channel_snapshots(&self) -> Vec<ChannelSnapshot> {
        Node::channel_snapshots(self)
    }

    fn open_channel(
        &self, _capacity_satoshis: u64,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + '_>> {
        Box::pin(async { Err("single node cannot open an internal channel".to_string()) })
    }

    fn block_height(&self) -> Pin<Box<dyn Future<Output = Result<u64, String>> + Send>> {
        let bitcoind = Arc::clone(&self.bitcoind);
        Box::pin(async move { bitcoind.get_block_height().await })
    }
}

impl HoldNode for DualNode {
    fn register_hold(&self, payment_hash: [u8; 32], amount_msat: u64) -> Result<String, String> {
        self.buyer.register_hold(payment_hash, amount_msat)
    }

    fn claim(&self, preimage: [u8; 32], payment_hash: [u8; 32]) -> Result<(), String> {
        self.buyer.claim(preimage, payment_hash)
    }

    fn fail(&self, payment_hash: [u8; 32]) -> Result<(), String> {
        self.buyer.fail(payment_hash)
    }

    fn pay(&self, bolt11: &str) -> Result<[u8; 32], String> {
        // Seller pays the buyer's hold invoice through the internal channel.
        // One LDK node cannot route a payment to itself.
        if !self.has_usable_route() {
            return Err(
                "no usable seller-to-buyer channel; POST /api/channel/open and mine confirmations"
                    .to_string(),
            );
        }
        self.seller.pay(bolt11)
    }

    fn subscribe(&self) -> broadcast::Receiver<WireEvent> {
        self.subscribe_merged()
    }

    fn node_pubkey(&self) -> String {
        self.buyer.node_pubkey.to_string()
    }

    fn buyer_pubkey(&self) -> String {
        self.buyer.node_pubkey.to_string()
    }

    fn seller_pubkey(&self) -> String {
        self.seller.node_pubkey.to_string()
    }

    fn channel_ids(&self) -> Vec<String> {
        DualNode::channel_ids(self)
    }

    fn channel_snapshots(&self) -> Vec<ChannelSnapshot> {
        DualNode::channel_snapshots(self)
    }

    fn peers_connected(&self) -> bool {
        DualNode::peers_connected(self)
    }

    fn open_channel(
        &self, capacity_satoshis: u64,
    ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + '_>> {
        Box::pin(async move { self.open_internal_channel(capacity_satoshis).await })
    }

    fn block_height(&self) -> Pin<Box<dyn Future<Output = Result<u64, String>> + Send>> {
        let bitcoind = Arc::clone(&self.buyer.bitcoind);
        Box::pin(async move { bitcoind.get_block_height().await })
    }
}

pub struct AppState<N: HoldNode> {
    pub node: N,
    pub store: Mutex<EscrowStore>,
    pub nostr_secret: [u8; 32],
}

pub fn router<N: HoldNode + 'static>(state: Arc<AppState<N>>) -> Router {
    Router::new()
        .route("/health/live", get(health))
        .route("/api/escrow/create", post(create_escrow::<N>))
        .route("/api/escrows", get(list_escrows::<N>))
        .route("/api/escrow/:id", get(get_escrow::<N>))
        .route("/api/escrow/:id/claim", post(claim_escrow::<N>))
        .route("/api/escrow/:id/fund", post(fund_escrow::<N>))
        .route("/api/escrow/:id/fail", post(fail_escrow::<N>))
        .route("/api/escrow/:id/dispute", post(dispute_escrow::<N>))
        .route("/api/invoice/pay", post(pay_invoice::<N>))
        .route("/api/channel/open", post(open_channel_route::<N>))
        .route("/api/channel/status", get(channel_status::<N>))
        .route("/api/node/info", get(node_info::<N>))
        .route("/api/node/channels", get(node_channels::<N>))
        .route("/api/ws", get(ws_events::<N>))
        .with_state(state)
        .layer(middleware::from_fn(allow_browser))
}

async fn allow_browser(req: Request, next: Next) -> Response {
    let origin = req.headers().get(header::ORIGIN).cloned();
    if req.method() == Method::OPTIONS {
        let mut res = Response::new(axum::body::Body::empty());
        *res.status_mut() = StatusCode::NO_CONTENT;
        apply_cors(res.headers_mut(), origin.as_ref());
        return res;
    }
    let mut res = next.run(req).await;
    apply_cors(res.headers_mut(), origin.as_ref());
    res
}

fn apply_cors(headers: &mut HeaderMap, origin: Option<&HeaderValue>) {
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        origin.cloned().unwrap_or_else(|| HeaderValue::from_static("*")),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("content-type, x-nostr-pubkey"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, OPTIONS"),
    );
}

async fn health() -> &'static str {
    "ok"
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateRequest {
    payment_hash: String,
    amount_msat: u64,
}

#[derive(Serialize)]
struct CreateResponse {
    id: String,
    payment_hash: String,
    bolt11_invoice: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimRequest {
    preimage: String,
}

async fn create_escrow<N: HoldNode>(
    State(state): State<Arc<AppState<N>>>, headers: HeaderMap, Json(body): Json<CreateRequest>,
) -> Result<Json<CreateResponse>, (StatusCode, String)> {
    let owner = owner_from_headers(&headers)?;
    let payment_hash = decode_fixed_hex(&body.payment_hash, "payment_hash")?;
    let bolt11_invoice =
        state.node.register_hold(payment_hash, body.amount_msat).map_err(bad_request)?;
    let id = new_id();
    let payment_hash_hex = hex::encode(payment_hash);
    state
        .store
        .lock()
        .expect("escrow store mutex poisoned")
        .insert(EscrowRecord {
            id: id.clone(),
            owner,
            payment_hash: payment_hash_hex.clone(),
            state: "open".to_string(),
            bolt11: bolt11_invoice.clone(),
            protocol_status: "secured".to_string(),
        })
        .map_err(bad_request)?;
    Ok(Json(CreateResponse { id, payment_hash: payment_hash_hex, bolt11_invoice }))
}

async fn list_escrows<N: HoldNode>(
    State(state): State<Arc<AppState<N>>>, headers: HeaderMap,
) -> Result<Json<Vec<EscrowRecord>>, (StatusCode, String)> {
    let owner = owner_from_headers(&headers)?;
    let records = state.store.lock().expect("escrow store mutex poisoned").list(&owner);
    Ok(Json(records))
}

async fn get_escrow<N: HoldNode>(
    State(state): State<Arc<AppState<N>>>, headers: HeaderMap, Path(id): Path<String>,
) -> Result<Json<EscrowRecord>, (StatusCode, String)> {
    let owner = owner_from_headers(&headers)?;
    let record = state
        .store
        .lock()
        .expect("escrow store mutex poisoned")
        .get_owned(&id, &owner)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "escrow not found".to_string()))?;
    Ok(Json(record))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PayRequest {
    bolt11: String,
}

#[derive(Serialize)]
struct PayResponse {
    payment_hash: String,
}

async fn pay_invoice<N: HoldNode>(
    State(state): State<Arc<AppState<N>>>, headers: HeaderMap, Json(body): Json<PayRequest>,
) -> Result<Json<PayResponse>, (StatusCode, String)> {
    let payer = owner_from_headers(&headers)?;
    let payment_hash = state.node.pay(&body.bolt11).map_err(bad_request)?;
    let payment_hash_hex = hex::encode(payment_hash);
    state
        .store
        .lock()
        .expect("escrow store mutex poisoned")
        .record_outgoing(&payer, &payment_hash_hex)
        .map_err(bad_request)?;
    Ok(Json(PayResponse { payment_hash: payment_hash_hex }))
}

async fn fund_escrow<N: HoldNode>(
    State(state): State<Arc<AppState<N>>>, headers: HeaderMap, Path(id): Path<String>,
    Json(body): Json<PayRequest>,
) -> Result<Json<PayResponse>, (StatusCode, String)> {
    let payer = owner_from_headers(&headers)?;
    let owns_invoice = {
        let store = state.store.lock().expect("escrow store mutex poisoned");
        let parsed = invoice::parse_bolt11(&body.bolt11).map_err(bad_request)?;
        let ph = hex::encode(payment_hash_bytes(&parsed));
        store.get_owned(&id, &payer).is_some()
            || store.list(&payer).iter().any(|record| record.payment_hash == ph)
    };
    if owns_invoice {
        return Err((
            StatusCode::BAD_REQUEST,
            "the invoice creator cannot fund it".to_string(),
        ));
    }
    let payment_hash = state.node.pay(&body.bolt11).map_err(bad_request)?;
    let payment_hash_hex = hex::encode(payment_hash);
    state
        .store
        .lock()
        .expect("escrow store mutex poisoned")
        .record_outgoing(&payer, &payment_hash_hex)
        .map_err(bad_request)?;
    Ok(Json(PayResponse { payment_hash: payment_hash_hex }))
}

async fn fail_escrow<N: HoldNode>(
    State(state): State<Arc<AppState<N>>>, headers: HeaderMap, Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let owner = owner_from_headers(&headers)?;
    let payment_hash = {
        let store = state.store.lock().expect("escrow store mutex poisoned");
        match store.get_owned(&id, &owner) {
            Some(record) => {
                if record.protocol_status == "settled" || record.protocol_status == "refunded" {
                    return Err((StatusCode::CONFLICT, "escrow is already settled or refunded".to_string()));
                }
                if record.protocol_status == "fail_submitted" || record.protocol_status == "disputed" {
                    return Err((StatusCode::CONFLICT, "escrow is already failed or disputed".to_string()));
                }
                decode_fixed_hex(&record.payment_hash, "payment_hash")?
            }
            None => return Err((StatusCode::NOT_FOUND, "escrow not found".to_string())),
        }
    };
    state.node.fail(payment_hash).map_err(bad_request)?;
    state
        .store
        .lock()
        .expect("escrow store mutex poisoned")
        .mark_fail_submitted(&id, &owner)
        .map_err(|_| (StatusCode::NOT_FOUND, "escrow not found".to_string()))?;
    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DisputeRequest {
    resolver: String,
}

#[derive(Serialize)]
struct DisputeResponse {
    gift_wraps: Vec<NostrEvent>,
}

async fn dispute_escrow<N: HoldNode>(
    State(state): State<Arc<AppState<N>>>, headers: HeaderMap, Path(id): Path<String>,
    Json(body): Json<DisputeRequest>,
) -> Result<Json<DisputeResponse>, (StatusCode, String)> {
    let owner = owner_from_headers(&headers)?;
    if body.resolver.len() != 64 || !body.resolver.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err((StatusCode::BAD_REQUEST, "resolver must be 64 hex characters".to_string()));
    }
    let payment_hash = {
        let store = state.store.lock().expect("escrow store mutex poisoned");
        match store.get_owned(&id, &owner) {
            Some(record) => {
                if record.protocol_status == "settled" || record.protocol_status == "refunded" {
                    return Err((StatusCode::CONFLICT, "escrow is already settled or refunded".to_string()));
                }
                if record.protocol_status == "disputed" {
                    return Err((StatusCode::CONFLICT, "escrow is already disputed".to_string()));
                }
                record.payment_hash
            }
            None => return Err((StatusCode::NOT_FOUND, "escrow not found".to_string())),
        }
    };
    let sender_pk = pubkey_from_secret(&state.nostr_secret);
    let wraps = wrap_payload(
        &state.nostr_secret,
        &sender_pk,
        &body.resolver,
        &id,
        [sender_pk.clone(), body.resolver.clone()],
        PayloadBody::Request {
            quote: id.clone(),
            private_terms: ARBITRATOR_SHARE_TERMS.to_string(),
            payment_hash: Some(payment_hash),
            name: Some(ARBITRATOR_SHARE_NAME.to_string()),
        },
    )
    .map_err(bad_request)?;
    state
        .store
        .lock()
        .expect("escrow store mutex poisoned")
        .mark_disputed(&id, &owner)
        .map_err(|_| (StatusCode::NOT_FOUND, "escrow not found".to_string()))?;
    Ok(Json(DisputeResponse { gift_wraps: wraps }))
}

#[derive(Serialize)]
struct NodeInfo {
    node_pubkey: String,
    buyer_pubkey: String,
    seller_pubkey: String,
    block_height: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenChannelRequest {
    capacity_satoshis: u64,
}

#[derive(Serialize)]
struct OpenChannelResponse {
    channel_id: String,
}

#[derive(Serialize)]
struct ChannelStatusResponse {
    peers_connected: bool,
    buyer_pubkey: String,
    seller_pubkey: String,
    channels: Vec<ChannelStatusItem>,
}

#[derive(Serialize)]
struct ChannelStatusItem {
    channel_id: String,
    ready: bool,
    usable: bool,
    counterparty: String,
}

async fn node_info<N: HoldNode>(
    State(state): State<Arc<AppState<N>>>, headers: HeaderMap,
) -> Result<Json<NodeInfo>, (StatusCode, String)> {
    let _owner = owner_from_headers(&headers)?;
    let block_height = state.node.block_height().await.map_err(bad_request)?;
    Ok(Json(NodeInfo {
        node_pubkey: state.node.node_pubkey(),
        buyer_pubkey: state.node.buyer_pubkey(),
        seller_pubkey: state.node.seller_pubkey(),
        block_height,
    }))
}

async fn open_channel_route<N: HoldNode>(
    State(state): State<Arc<AppState<N>>>, headers: HeaderMap, Json(body): Json<OpenChannelRequest>,
) -> Result<Json<OpenChannelResponse>, (StatusCode, String)> {
    let _owner = owner_from_headers(&headers)?;
    let channel_id = state.node.open_channel(body.capacity_satoshis).await.map_err(bad_request)?;
    Ok(Json(OpenChannelResponse { channel_id }))
}

async fn channel_status<N: HoldNode>(
    State(state): State<Arc<AppState<N>>>, headers: HeaderMap,
) -> Result<Json<ChannelStatusResponse>, (StatusCode, String)> {
    let _owner = owner_from_headers(&headers)?;
    let channels = state
        .node
        .channel_snapshots()
        .into_iter()
        .map(|snapshot| ChannelStatusItem {
            channel_id: snapshot.channel_id,
            ready: snapshot.ready,
            usable: snapshot.usable,
            counterparty: snapshot.counterparty,
        })
        .collect();
    Ok(Json(ChannelStatusResponse {
        peers_connected: state.node.peers_connected(),
        buyer_pubkey: state.node.buyer_pubkey(),
        seller_pubkey: state.node.seller_pubkey(),
        channels,
    }))
}

async fn node_channels<N: HoldNode>(
    State(state): State<Arc<AppState<N>>>, headers: HeaderMap,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    let _owner = owner_from_headers(&headers)?;
    Ok(Json(state.node.channel_ids()))
}

async fn claim_escrow<N: HoldNode>(
    State(state): State<Arc<AppState<N>>>, headers: HeaderMap, Path(id): Path<String>,
    Json(body): Json<ClaimRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let owner = owner_from_headers(&headers)?;
    let preimage = decode_fixed_hex(&body.preimage, "preimage")?;
    let stored_hash = {
        let store = state.store.lock().expect("escrow store mutex poisoned");
        match store.get(&id) {
            Some(record) if record.owner == owner => record.payment_hash,
            _ => return Err((StatusCode::NOT_FOUND, "escrow not found".to_string())),
        }
    };
    let computed = sha256_hex(&preimage);
    if computed != stored_hash {
        return Err((StatusCode::BAD_REQUEST, "preimage does not match payment_hash".to_string()));
    }
    let payment_hash = decode_fixed_hex(&stored_hash, "payment_hash")?;
    state.node.claim(preimage, payment_hash).map_err(bad_request)?;
    state
        .store
        .lock()
        .expect("escrow store mutex poisoned")
        .mark_claim_submitted(&id, &owner)
        .map_err(|_| (StatusCode::NOT_FOUND, "escrow not found".to_string()))?;
    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
struct WsQuery {
    pubkey: Option<String>,
}

async fn ws_events<N: HoldNode + 'static>(
    State(state): State<Arc<AppState<N>>>,
    headers: HeaderMap,
    Query(query): Query<WsQuery>,
    upgrade: WebSocketUpgrade,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Browsers cannot set X-Nostr-Pubkey on WebSocket; the query string is the client path.
    let owner = owner_from_headers_or_query(&headers, query.pubkey.as_deref())?;
    Ok(upgrade.on_upgrade(move |socket| stream_events(socket, state, owner)))
}

async fn stream_events<N: HoldNode>(
    mut socket: WebSocket, state: Arc<AppState<N>>, owner: String,
) {
    let mut rx = state.node.subscribe();
    loop {
        let event = match rx.recv().await {
            Ok(event) => event,
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        };
        let visible = event.kind == "ChannelReady" || match event.payment_hash.as_deref() {
            Some(payment_hash) => state
                .store
                .lock()
                .expect("escrow store mutex poisoned")
                .visible_hash(&owner, payment_hash),
            None => false,
        };
        if !visible {
            continue;
        }
        let payload = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string());
        if socket.send(Message::Text(payload)).await.is_err() {
            break;
        }
    }
}

fn validate_pubkey(owner: &str) -> Result<String, (StatusCode, String)> {
    if owner.len() != 64 || !owner.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err((StatusCode::BAD_REQUEST, "X-Nostr-Pubkey must be 64 hex characters".to_string()));
    }
    Ok(owner.to_ascii_lowercase())
}

fn owner_from_headers(headers: &HeaderMap) -> Result<String, (StatusCode, String)> {
    let value = headers
        .get(NOSTR_PUBKEY_HEADER)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing X-Nostr-Pubkey".to_string()))?;
    let owner = value
        .to_str()
        .map_err(|_| (StatusCode::BAD_REQUEST, "X-Nostr-Pubkey is not ascii".to_string()))?;
    validate_pubkey(owner)
}

fn owner_from_headers_or_query(
    headers: &HeaderMap,
    query_pubkey: Option<&str>,
) -> Result<String, (StatusCode, String)> {
    if let Some(pk) = query_pubkey.filter(|value| !value.is_empty()) {
        return validate_pubkey(pk);
    }
    owner_from_headers(headers)
}

fn decode_fixed_hex(value: &str, field: &str) -> Result<[u8; 32], (StatusCode, String)> {
    let bytes = hex::decode(value).map_err(|_| {
        (StatusCode::BAD_REQUEST, format!("{field} must be hex"))
    })?;
    let array: [u8; 32] = bytes.try_into().map_err(|_| {
        (StatusCode::BAD_REQUEST, format!("{field} must be 32 bytes"))
    })?;
    Ok(array)
}

fn bad_request(message: String) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, message)
}

fn sha256_hex(bytes: &[u8]) -> String {
    use bitcoin::hashes::Hash;
    hex::encode(bitcoin::hashes::sha256::Hash::hash(bytes).to_byte_array())
}

fn new_id() -> String {
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;

    struct FakeNode {
        claims: Mutex<Vec<[u8; 32]>>,
        fails: Arc<Mutex<Vec<[u8; 32]>>>,
        tx: broadcast::Sender<WireEvent>,
    }

    impl HoldNode for FakeNode {
        fn register_hold(&self, payment_hash: [u8; 32], _amount_msat: u64) -> Result<String, String> {
            Ok(format!("lnbcrt1{}", hex::encode(payment_hash)))
        }

        fn claim(&self, preimage: [u8; 32], _payment_hash: [u8; 32]) -> Result<(), String> {
            self.claims.lock().expect("claims mutex poisoned").push(preimage);
            Ok(())
        }

        fn fail(&self, payment_hash: [u8; 32]) -> Result<(), String> {
            self.fails.lock().expect("fails mutex poisoned").push(payment_hash);
            Ok(())
        }

        fn pay(&self, bolt11: &str) -> Result<[u8; 32], String> {
            let invoice = invoice::parse_bolt11(bolt11)?;
            Ok(payment_hash_bytes(&invoice))
        }

        fn subscribe(&self) -> broadcast::Receiver<WireEvent> {
            self.tx.subscribe()
        }

        fn node_pubkey(&self) -> String {
            "02".repeat(33)
        }

        fn channel_ids(&self) -> Vec<String> {
            vec!["channel-1".to_string()]
        }

        fn channel_snapshots(&self) -> Vec<ChannelSnapshot> {
            vec![ChannelSnapshot {
                channel_id: "channel-1".to_string(),
                ready: true,
                usable: true,
                counterparty: "02".repeat(33),
            }]
        }

        fn open_channel(
            &self, _capacity_satoshis: u64,
        ) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + '_>> {
            Box::pin(async { Ok("channel-1".to_string()) })
        }

        fn block_height(&self) -> Pin<Box<dyn Future<Output = Result<u64, String>> + Send>> {
            Box::pin(async { Ok(42) })
        }
    }

    fn app(dir: &std::path::Path) -> Router {
        app_with_fails(dir).0
    }

    fn app_with_fails(dir: &std::path::Path) -> (Router, Arc<Mutex<Vec<[u8; 32]>>>) {
        let (tx, _) = broadcast::channel(8);
        let fails = Arc::new(Mutex::new(Vec::new()));
        let state = Arc::new(AppState {
            node: FakeNode {
                claims: Mutex::new(Vec::new()),
                fails: Arc::clone(&fails),
                tx,
            },
            store: Mutex::new(EscrowStore::open(dir.join("escrows.json")).unwrap()),
            nostr_secret: [9u8; 32],
        });
        (router(state), fails)
    }

    fn pubkey(seed: u8) -> String {
        hex::encode([seed; 32])
    }

    #[test]
    fn websocket_accepts_pubkey_query_without_header() {
        let headers = HeaderMap::new();
        let owner = owner_from_headers_or_query(&headers, Some(&pubkey(0xab))).unwrap();
        assert_eq!(owner, pubkey(0xab));
        assert!(owner_from_headers(&headers).is_err());
    }

    #[tokio::test]
    async fn health_allows_browser_origin() {
        let dir = std::env::temp_dir().join("ponthtlc-http-cors");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let response = app(&dir)
            .oneshot(
                Request::builder()
                    .uri("/health/live")
                    .header("origin", "http://localhost:5173")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get("access-control-allow-origin").unwrap(),
            "http://localhost:5173"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn health_is_ok() {
        let dir = std::env::temp_dir().join("ponthtlc-http-health");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let response = app(&dir)
            .oneshot(Request::builder().uri("/health/live").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(&body[..], b"ok");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn escrows_are_isolated_by_pubkey() {
        let dir = std::env::temp_dir().join("ponthtlc-http-isolation");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let application = app(&dir);
        let owner_a = pubkey(0xab);
        let owner_b = pubkey(0xcd);
        let hash_a = "11".repeat(32);
        let hash_b = "22".repeat(32);
        let created = application
            .clone()
            .oneshot(json_request("POST", "/api/escrow/create", &owner_a, &format!(
                "{{\"payment_hash\":\"{hash_a}\",\"amount_msat\":50000000}}"
            )))
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::OK);
        let created_body = to_bytes(created.into_body(), usize::MAX).await.unwrap();
        let created_json: serde_json::Value = serde_json::from_slice(&created_body).unwrap();
        assert!(created_json["bolt11_invoice"].as_str().unwrap().contains(&hash_a));
        let id = created_json["id"].as_str().unwrap().to_string();
        let hidden = application
            .clone()
            .oneshot(json_request("GET", &format!("/api/escrow/{id}"), &owner_b, ""))
            .await
            .unwrap();
        assert_eq!(hidden.status(), StatusCode::NOT_FOUND);
        let visible = application
            .clone()
            .oneshot(json_request("GET", &format!("/api/escrow/{id}"), &owner_a, ""))
            .await
            .unwrap();
        assert_eq!(visible.status(), StatusCode::OK);
        let created_b = application
            .clone()
            .oneshot(json_request("POST", "/api/escrow/create", &owner_b, &format!(
                "{{\"payment_hash\":\"{hash_b}\",\"amount_msat\":50000000}}"
            )))
            .await
            .unwrap();
        assert_eq!(created_b.status(), StatusCode::OK);
        let listed = application
            .oneshot(json_request("GET", "/api/escrows", &owner_a, ""))
            .await
            .unwrap();
        assert_eq!(listed.status(), StatusCode::OK);
        let body = to_bytes(listed.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains(&hash_a));
        assert!(!text.contains(&hash_b));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn claim_does_not_write_preimage() {
        let dir = std::env::temp_dir().join("ponthtlc-http-claim");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let application = app(&dir);
        let owner = pubkey(0x11);
        let preimage = "ee".repeat(32);
        let raw = hex::decode(&preimage).unwrap();
        let payment_hash = sha256_hex(&raw);
        let created = application
            .clone()
            .oneshot(json_request("POST", "/api/escrow/create", &owner, &format!(
                "{{\"payment_hash\":\"{payment_hash}\",\"amount_msat\":1000}}"
            )))
            .await
            .unwrap();
        let created_body = to_bytes(created.into_body(), usize::MAX).await.unwrap();
        let created_json: serde_json::Value = serde_json::from_slice(&created_body).unwrap();
        let id = created_json["id"].as_str().unwrap();
        let claimed = application
            .oneshot(json_request(
                "POST",
                &format!("/api/escrow/{id}/claim"),
                &owner,
                &format!("{{\"preimage\":\"{preimage}\"}}"),
            ))
            .await
            .unwrap();
        assert_eq!(claimed.status(), StatusCode::OK);
        assert!(!crate::tenancy::tree_contains_secret(&dir, &preimage, &raw));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn pay_records_the_hash_for_the_payer_only() {
        let dir = std::env::temp_dir().join("ponthtlc-http-pay");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let application = app(&dir);
        let payer = pubkey(0x44);
        let other = pubkey(0x55);
        let payment_hash = [0x66u8; 32];
        let keys = lightning::sign::KeysManager::new(&[4u8; 32], 1, 0, true);
        let invoice = invoice::build_hold_invoice(
            &keys,
            payment_hash,
            lightning::types::payment::PaymentSecret([8u8; 32]),
            1000,
            lightning_invoice::Currency::Regtest,
        )
        .unwrap()
        .to_string();
        let paid = application
            .clone()
            .oneshot(json_request(
                "POST",
                "/api/invoice/pay",
                &payer,
                &format!("{{\"bolt11\":\"{invoice}\"}}"),
            ))
            .await
            .unwrap();
        assert_eq!(paid.status(), StatusCode::OK);
        let paid_body = to_bytes(paid.into_body(), usize::MAX).await.unwrap();
        let paid_json: serde_json::Value = serde_json::from_slice(&paid_body).unwrap();
        let hash_hex = hex::encode(payment_hash);
        assert_eq!(paid_json["payment_hash"].as_str().unwrap(), hash_hex);
        let stored = std::fs::read_to_string(dir.join("escrows.json")).unwrap();
        assert!(stored.contains(&payer));
        assert!(stored.contains(&hash_hex));
        assert!(!stored.contains(&other));
        let listed = application
            .oneshot(json_request("GET", "/api/escrows", &payer, ""))
            .await
            .unwrap();
        let listed_body = to_bytes(listed.into_body(), usize::MAX).await.unwrap();
        let listed_text = String::from_utf8(listed_body.to_vec()).unwrap();
        assert!(!listed_text.contains(&hash_hex));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn fail_calls_the_node_and_does_not_write_a_preimage() {
        let dir = std::env::temp_dir().join("ponthtlc-http-fail");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (application, fails) = app_with_fails(&dir);
        let owner = pubkey(0x77);
        let payment_hash = "44".repeat(32);
        let preimage = "ee".repeat(32);
        let created = application
            .clone()
            .oneshot(json_request("POST", "/api/escrow/create", &owner, &format!(
                "{{\"payment_hash\":\"{payment_hash}\",\"amount_msat\":1000}}"
            )))
            .await
            .unwrap();
        let created_body = to_bytes(created.into_body(), usize::MAX).await.unwrap();
        let created_json: serde_json::Value = serde_json::from_slice(&created_body).unwrap();
        let id = created_json["id"].as_str().unwrap();
        let failed = application
            .clone()
            .oneshot(json_request("POST", &format!("/api/escrow/{id}/fail"), &owner, "{}"))
            .await
            .unwrap();
        assert_eq!(failed.status(), StatusCode::OK);
        let expected: [u8; 32] = hex::decode(&payment_hash).unwrap().try_into().unwrap();
        assert_eq!(fails.lock().expect("fails mutex poisoned").as_slice(), &[expected]);
        let listed = application
            .oneshot(json_request("GET", &format!("/api/escrow/{id}"), &owner, ""))
            .await
            .unwrap();
        let body = to_bytes(listed.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("fail_submitted"));
        let raw = hex::decode(&preimage).unwrap();
        assert!(!crate::tenancy::tree_contains_secret(&dir, &preimage, &raw));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn dispute_requests_the_arbitrator_share_without_a_preimage() {
        let dir = std::env::temp_dir().join("ponthtlc-http-dispute");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let application = app(&dir);
        let owner = pubkey(0x88);
        let payment_hash = "55".repeat(32);
        let created = application
            .clone()
            .oneshot(json_request("POST", "/api/escrow/create", &owner, &format!(
                "{{\"payment_hash\":\"{payment_hash}\",\"amount_msat\":1000}}"
            )))
            .await
            .unwrap();
        let created_body = to_bytes(created.into_body(), usize::MAX).await.unwrap();
        let created_json: serde_json::Value = serde_json::from_slice(&created_body).unwrap();
        let id = created_json["id"].as_str().unwrap();
        let resolver = "ab".repeat(32);
        let disputed = application
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/api/escrow/{id}/dispute"),
                &owner,
                &format!("{{\"resolver\":\"{resolver}\"}}"),
            ))
            .await
            .unwrap();
        assert_eq!(disputed.status(), StatusCode::OK);
        let body = to_bytes(disputed.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["gift_wraps"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["gift_wraps"][0]["kind"].as_u64().unwrap(), 1059);
        assert!(!text.contains(&"ee".repeat(32)));
        let listed = application
            .oneshot(json_request("GET", &format!("/api/escrow/{id}"), &owner, ""))
            .await
            .unwrap();
        let listed_body = to_bytes(listed.into_body(), usize::MAX).await.unwrap();
        let listed_text = String::from_utf8(listed_body.to_vec()).unwrap();
        assert!(listed_text.contains("\"protocol_status\":\"disputed\""));
        assert!(!listed_text.contains(&"ee".repeat(32)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn node_routes_return_host_info() {
        let dir = std::env::temp_dir().join("ponthtlc-http-node");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let application = app(&dir);
        let owner = pubkey(0x99);
        let info = application
            .clone()
            .oneshot(json_request("GET", "/api/node/info", &owner, ""))
            .await
            .unwrap();
        assert_eq!(info.status(), StatusCode::OK);
        let info_body = to_bytes(info.into_body(), usize::MAX).await.unwrap();
        let info_json: serde_json::Value = serde_json::from_slice(&info_body).unwrap();
        assert_eq!(info_json["block_height"].as_u64().unwrap(), 42);
        assert!(!info_json["node_pubkey"].as_str().unwrap().is_empty());
        assert_eq!(
            info_json["buyer_pubkey"].as_str().unwrap(),
            info_json["node_pubkey"].as_str().unwrap()
        );
        let channels = application
            .oneshot(json_request("GET", "/api/node/channels", &owner, ""))
            .await
            .unwrap();
        let channels_body = to_bytes(channels.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(channels_body.to_vec()).unwrap();
        assert!(text.contains("channel-1"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn channel_open_returns_an_id() {
        let dir = std::env::temp_dir().join("ponthtlc-http-channel-open");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let application = app(&dir);
        let owner = pubkey(0xaa);
        let opened = application
            .clone()
            .oneshot(json_request(
                "POST",
                "/api/channel/open",
                &owner,
                "{\"capacity_satoshis\":1000000}",
            ))
            .await
            .unwrap();
        assert_eq!(opened.status(), StatusCode::OK);
        let opened_body = to_bytes(opened.into_body(), usize::MAX).await.unwrap();
        let opened_json: serde_json::Value = serde_json::from_slice(&opened_body).unwrap();
        assert_eq!(opened_json["channel_id"].as_str().unwrap(), "channel-1");
        let status = application
            .oneshot(json_request("GET", "/api/channel/status", &owner, ""))
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let status_body = to_bytes(status.into_body(), usize::MAX).await.unwrap();
        let status_json: serde_json::Value = serde_json::from_slice(&status_body).unwrap();
        assert_eq!(status_json["peers_connected"].as_bool().unwrap(), true);
        assert_eq!(status_json["channels"][0]["channel_id"].as_str().unwrap(), "channel-1");
        assert_eq!(status_json["channels"][0]["usable"].as_bool().unwrap(), true);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn claim_rejects_preimage_that_does_not_match_hash() {
        let dir = std::env::temp_dir().join("ponthtlc-http-claim-mismatch");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let application = app(&dir);
        let owner = pubkey(0x22);
        let payment_hash = "33".repeat(32);
        let created = application
            .clone()
            .oneshot(json_request("POST", "/api/escrow/create", &owner, &format!(
                "{{\"payment_hash\":\"{payment_hash}\",\"amount_msat\":1000}}"
            )))
            .await
            .unwrap();
        let created_body = to_bytes(created.into_body(), usize::MAX).await.unwrap();
        let created_json: serde_json::Value = serde_json::from_slice(&created_body).unwrap();
        let id = created_json["id"].as_str().unwrap();
        let claimed = application
            .clone()
            .oneshot(json_request(
                "POST",
                &format!("/api/escrow/{id}/claim"),
                &owner,
                "{\"preimage\":\"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee\"}",
            ))
            .await
            .unwrap();
        assert_eq!(claimed.status(), StatusCode::BAD_REQUEST);
        let listed = application
            .oneshot(json_request("GET", "/api/escrows", &owner, ""))
            .await
            .unwrap();
        let body = to_bytes(listed.into_body(), usize::MAX).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("\"state\":\"open\""));
        assert!(!text.contains("claim_submitted"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn json_request(method: &str, uri: &str, owner: &str, body: &str) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .header("x-nostr-pubkey", owner)
            .body(Body::from(body.to_string()))
            .unwrap()
    }
}
