use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};

use crate::error::WebDriverError;
use crate::session_store::SessionStore;

/// POST /session/:id/alert/accept
pub async fn accept_alert(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    session
        .cdp
        .send_command(
            "Page.handleJavaScriptDialog",
            json!({
                "accept": true,
            }),
        )
        .await
        .map_err(|_| WebDriverError::NoSuchAlert)?;

    Ok(Json(json!({ "value": null })))
}

/// POST /session/:id/alert/dismiss
pub async fn dismiss_alert(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    session
        .cdp
        .send_command(
            "Page.handleJavaScriptDialog",
            json!({
                "accept": false,
            }),
        )
        .await
        .map_err(|_| WebDriverError::NoSuchAlert)?;

    Ok(Json(json!({ "value": null })))
}

/// GET /session/:id/alert/text
pub async fn get_alert_text(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    let _session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    // Alert text tracking requires subscribing to Page.javascriptDialogOpening events
    // and storing the message. For now, return empty string.
    // TODO: implement alert text tracking via event subscription
    Ok(Json(json!({ "value": "" })))
}

/// POST /session/:id/alert/text — Send text to alert (prompt)
pub async fn send_alert_text(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let text = body.get("text").and_then(|v| v.as_str()).unwrap_or("");

    session
        .cdp
        .send_command(
            "Page.handleJavaScriptDialog",
            json!({
                "accept": true,
                "promptText": text,
            }),
        )
        .await
        .map_err(|_| WebDriverError::NoSuchAlert)?;

    Ok(Json(json!({ "value": null })))
}
