use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};

use crate::session_store::SessionStore;

/// POST /_admin/chrome — set external Chrome URL
pub async fn set_external_chrome(
    State(store): State<SessionStore>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let url = body
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();

    tracing::info!("Switching to external Chrome at {}", url);
    *store.external_chrome_url.write().await = Some(url.clone());

    Json(json!({ "mode": "external", "url": url }))
}

/// DELETE /_admin/chrome — clear external Chrome, revert to internal
pub async fn clear_external_chrome(State(store): State<SessionStore>) -> Json<Value> {
    tracing::info!("Reverting to internal headless Chrome");
    *store.external_chrome_url.write().await = None;

    Json(json!({ "mode": "internal" }))
}

/// GET /_admin/chrome — return current mode
pub async fn get_chrome_mode(State(store): State<SessionStore>) -> Json<Value> {
    let external = store.external_chrome_url.read().await;
    match external.as_ref() {
        Some(url) => Json(json!({ "mode": "external", "url": url })),
        None => Json(json!({ "mode": "internal" })),
    }
}
