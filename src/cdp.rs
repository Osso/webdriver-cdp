use anyhow::{Context, Result};
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

type PendingMap = Arc<DashMap<i64, oneshot::Sender<Result<Value, String>>>>;

/// A CDP connection with an event multiplexer.
/// Two background tasks own the WebSocket:
/// - Reader: Messages with "id" → routed to pending oneshot channels
///           Messages with "method" → broadcast to event subscribers
/// - Writer: Forwards raw JSON strings from the mpsc channel to the WS
#[allow(dead_code)]
pub struct CdpSession {
    msg_tx: mpsc::Sender<String>,
    pending: PendingMap,
    event_tx: broadcast::Sender<Value>,
    next_id: AtomicI64,
}

fn spawn_writer_task(
    mut ws_sink: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    mut msg_rx: mpsc::Receiver<String>,
) {
    tokio::spawn(async move {
        while let Some(msg) = msg_rx.recv().await {
            if ws_sink.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });
}

fn route_cdp_response(pending: &PendingMap, id: i64, val: &Value) {
    if let Some((_, tx)) = pending.remove(&id) {
        if let Some(error) = val.get("error") {
            let msg = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("CDP error")
                .to_string();
            let _ = tx.send(Err(msg));
        } else {
            let result = val.get("result").cloned().unwrap_or(json!({}));
            let _ = tx.send(Ok(result));
        }
    }
}

fn spawn_reader_task(
    mut ws_stream: futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    pending: PendingMap,
    event_tx: broadcast::Sender<Value>,
) {
    tokio::spawn(async move {
        while let Some(msg) = ws_stream.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(val) = serde_json::from_str::<Value>(&text) {
                        if let Some(id) = val.get("id").and_then(|v| v.as_i64()) {
                            route_cdp_response(&pending, id, &val);
                        } else if val.get("method").is_some() {
                            let _ = event_tx.send(val);
                        }
                    }
                }
                Ok(Message::Close(_)) | Err(_) => break,
                _ => {}
            }
        }
    });
}

fn translate_cdp_error(cdp_error: String) -> crate::error::WebDriverError {
    if cdp_error.contains("Cannot find context")
        || cdp_error.contains("Execution context was destroyed")
    {
        crate::error::WebDriverError::NoSuchFrame
    } else if cdp_error.contains("Could not find object with given id") {
        crate::error::WebDriverError::StaleElementReference
    } else {
        crate::error::WebDriverError::UnknownError(cdp_error)
    }
}

impl CdpSession {
    /// Connect to a Chrome target's WebSocket URL and start the multiplexer.
    pub async fn connect(ws_url: &str) -> Result<Self> {
        let (ws, _) = tokio_tungstenite::connect_async(ws_url)
            .await
            .context("Failed to connect to CDP WebSocket")?;

        let (ws_sink, ws_stream) = ws.split();
        let pending: PendingMap = Arc::new(DashMap::new());
        let (event_tx, _) = broadcast::channel(256);
        let (msg_tx, msg_rx) = mpsc::channel::<String>(64);

        spawn_writer_task(ws_sink, msg_rx);
        spawn_reader_task(ws_stream, pending.clone(), event_tx.clone());

        Ok(Self {
            msg_tx,
            pending,
            event_tx,
            next_id: AtomicI64::new(1),
        })
    }

    /// Send a CDP command and wait for the response.
    pub async fn send_command(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Value, crate::error::WebDriverError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let msg = json!({ "id": id, "method": method, "params": params });

        let (tx, rx) = oneshot::channel();
        self.pending.insert(id, tx);

        self.msg_tx.send(msg.to_string()).await.map_err(|_| {
            self.pending.remove(&id);
            crate::error::WebDriverError::UnknownError("CDP connection closed".into())
        })?;

        match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
            Ok(Ok(Ok(result))) => Ok(result),
            Ok(Ok(Err(cdp_error))) => Err(translate_cdp_error(cdp_error)),
            Ok(Err(_)) => Err(crate::error::WebDriverError::UnknownError(
                "Response channel dropped".into(),
            )),
            Err(_) => {
                self.pending.remove(&id);
                Err(crate::error::WebDriverError::Timeout)
            }
        }
    }

    /// Subscribe to CDP events.
    #[allow(dead_code)]
    pub fn subscribe_events(&self) -> broadcast::Receiver<Value> {
        self.event_tx.subscribe()
    }
}
