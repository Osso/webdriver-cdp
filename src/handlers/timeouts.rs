use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};

use crate::error::WebDriverError;
use crate::session_store::SessionStore;

/// GET /session/:id/timeouts
pub async fn get_timeouts(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    Ok(Json(json!({
        "value": {
            "implicit": session.implicit_wait_ms,
            "pageLoad": session.page_load_timeout_ms,
            "script": session.script_timeout_ms,
        }
    })))
}

/// POST /session/:id/timeouts
pub async fn set_timeouts(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, WebDriverError> {
    let mut session = store
        .sessions
        .get_mut(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    if let Some(implicit) = body.get("implicit").and_then(|v| v.as_u64()) {
        session.implicit_wait_ms = implicit;
    }
    if let Some(page_load) = body.get("pageLoad").and_then(|v| v.as_u64()) {
        session.page_load_timeout_ms = page_load;
    }
    if let Some(script) = body.get("script").and_then(|v| v.as_u64()) {
        session.script_timeout_ms = script;
    }

    Ok(Json(json!({ "value": null })))
}
