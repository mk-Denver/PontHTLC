use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use ponthtlc_protocol::event::NostrEvent;
use serde_json::{json, Value};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;

type WsStream = WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

pub struct Relay {
    url: String,
    writer: Arc<Mutex<futures_util::stream::SplitSink<WsStream, Message>>>,
    reader: Arc<Mutex<futures_util::stream::SplitStream<WsStream>>>,
}

impl Relay {
    pub async fn connect(url: &str) -> Result<Self, String> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(url)
            .await
            .map_err(|e| format!("relay connect failed: {e}"))?;
        let (writer, reader) = ws_stream.split();
        Ok(Self {
            url: url.to_string(),
            writer: Arc::new(Mutex::new(writer)),
            reader: Arc::new(Mutex::new(reader)),
        })
    }

    pub async fn publish(&self, event: &NostrEvent) -> Result<(), String> {
        let msg = json!(["EVENT", event]);
        let text = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        self.writer
            .lock()
            .await
            .send(Message::Text(text))
            .await
            .map_err(|e| format!("send failed: {e}"))
    }

    pub async fn subscribe(
        &self,
        subscription_id: &str,
        kinds: &[u32],
        authors: &[&str],
        p_tags: &[&str],
        e_tags: &[&str],
        since: Option<u64>,
        until: Option<u64>,
        limit: Option<u64>,
    ) -> Result<(), String> {
        let mut filter = serde_json::Map::new();
        if !kinds.is_empty() {
            filter.insert("kinds".to_string(), json!(kinds));
        }
        if !authors.is_empty() {
            filter.insert("authors".to_string(), json!(authors));
        }
        if !p_tags.is_empty() {
            filter.insert("#p".to_string(), json!(p_tags));
        }
        if !e_tags.is_empty() {
            filter.insert("#e".to_string(), json!(e_tags));
        }
        if let Some(s) = since {
            filter.insert("since".to_string(), json!(s));
        }
        if let Some(u) = until {
            filter.insert("until".to_string(), json!(u));
        }
        if let Some(l) = limit {
            filter.insert("limit".to_string(), json!(l));
        }
        let msg = json!(["REQ", subscription_id, filter]);
        let text = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        self.writer
            .lock()
            .await
            .send(Message::Text(text))
            .await
            .map_err(|e| format!("send failed: {e}"))
    }

    pub async fn unsubscribe(&self, subscription_id: &str) -> Result<(), String> {
        let msg = json!(["CLOSE", subscription_id]);
        let text = serde_json::to_string(&msg).map_err(|e| e.to_string())?;
        self.writer
            .lock()
            .await
            .send(Message::Text(text))
            .await
            .map_err(|e| format!("send failed: {e}"))
    }

    pub async fn recv(&self) -> Result<Option<(String, NostrEvent)>, String> {
        loop {
            let msg = {
                let mut reader = self.reader.lock().await;
                reader.next().await
            };
            match msg {
                Some(Ok(Message::Text(text))) => {
                    let msg: Value = serde_json::from_str(&text)
                        .map_err(|e| format!("parse: {e}"))?;
                    if !msg.is_array() {
                        continue;
                    }
                    let arr = msg.as_array().unwrap();
                    if arr.is_empty() {
                        continue;
                    }
                    match arr[0].as_str() {
                        Some("EVENT") => {
                            if arr.len() < 3 {
                                continue;
                            }
                            let sub_id = arr[1]
                                .as_str()
                                .unwrap_or("")
                                .to_string();
                            let event: NostrEvent = serde_json::from_value(arr[2].clone())
                                .map_err(|e| format!("parse event: {e}"))?;
                            return Ok(Some((sub_id, event)));
                        }
                        _ => continue,
                    }
                }
                Some(Ok(Message::Binary(_))) => continue,
                Some(Ok(Message::Ping(_))) => continue,
                Some(Ok(Message::Pong(_))) => continue,
                Some(Ok(Message::Close(_))) => return Ok(None),
                Some(Ok(_)) => continue,
                Some(Err(e)) => return Err(format!("ws error: {e}")),
                None => return Ok(None),
            }
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}