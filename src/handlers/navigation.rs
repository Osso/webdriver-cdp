use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};

use crate::error::WebDriverError;
use crate::session_store::SessionStore;

fn extract_string_result(result: &Value) -> &str {
    result
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

/// POST /session/:id/url — Navigate to URL
pub async fn navigate(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let url = body
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| WebDriverError::InvalidArgument("Missing 'url' parameter".into()))?;

    // pageLoadStrategy: none — return immediately after navigation starts
    session
        .cdp
        .send_command("Page.navigate", json!({ "url": url }))
        .await?;

    Ok(Json(json!({ "value": null })))
}

/// GET /session/:id/url
pub async fn get_url(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let result = session
        .cdp
        .send_command(
            "Runtime.evaluate",
            json!({
                "expression": "window.location.href",
                "returnByValue": true,
            }),
        )
        .await?;

    Ok(Json(json!({ "value": extract_string_result(&result) })))
}

/// GET /session/:id/title
pub async fn get_title(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let result = session
        .cdp
        .send_command(
            "Runtime.evaluate",
            json!({
                "expression": "document.title",
                "returnByValue": true,
            }),
        )
        .await?;

    Ok(Json(json!({ "value": extract_string_result(&result) })))
}

/// POST /session/:id/back
pub async fn back(
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
            "Runtime.evaluate",
            json!({
                "expression": "history.back()",
                "returnByValue": true,
            }),
        )
        .await?;

    Ok(Json(json!({ "value": null })))
}

/// POST /session/:id/forward
pub async fn forward(
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
            "Runtime.evaluate",
            json!({
                "expression": "history.forward()",
                "returnByValue": true,
            }),
        )
        .await?;

    Ok(Json(json!({ "value": null })))
}

/// POST /session/:id/refresh
pub async fn refresh(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    session.cdp.send_command("Page.reload", json!({})).await?;

    Ok(Json(json!({ "value": null })))
}

/// GET /session/:id/source
pub async fn get_source(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let result = session
        .cdp
        .send_command(
            "Runtime.evaluate",
            json!({
                "expression": "document.documentElement.outerHTML",
                "returnByValue": true,
            }),
        )
        .await?;

    Ok(Json(json!({ "value": extract_string_result(&result) })))
}
