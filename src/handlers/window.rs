use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};

use crate::error::WebDriverError;
use crate::session_store::SessionStore;

/// GET /session/:id/window — Get current window handle
pub async fn get_window_handle(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    Ok(Json(json!({ "value": session.target_id })))
}

/// GET /session/:id/window/handles — Get all window handles
pub async fn get_window_handles(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    // Single tab per session — return just this one
    Ok(Json(json!({ "value": [session.target_id] })))
}

/// POST /session/:id/window — Switch to window (no-op for single tab)
pub async fn switch_to_window(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let handle = body
        .get("handle")
        .and_then(|v| v.as_str())
        .ok_or_else(|| WebDriverError::InvalidArgument("Missing 'handle'".into()))?;

    if handle != session.target_id {
        return Err(WebDriverError::NoSuchWindow);
    }

    Ok(Json(json!({ "value": null })))
}

/// DELETE /session/:id/window — Close current window
pub async fn close_window(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .remove(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let _ = crate::chrome::close_target(store.chrome_port, &session.1.target_id).await;

    Ok(Json(json!({ "value": [] })))
}

/// POST /session/:id/window/rect — Set window rect (size/position)
pub async fn set_window_rect(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let width = body.get("width").and_then(|v| v.as_i64()).unwrap_or(1800);
    let height = body.get("height").and_then(|v| v.as_i64()).unwrap_or(1200);

    session
        .cdp
        .send_command(
            "Emulation.setDeviceMetricsOverride",
            json!({
                "width": width,
                "height": height,
                "deviceScaleFactor": 1,
                "mobile": false,
            }),
        )
        .await?;

    Ok(Json(json!({
        "value": { "x": 0, "y": 0, "width": width, "height": height }
    })))
}

async fn query_window_size(
    store: &SessionStore,
    session_id: &str,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let result = session.cdp.send_command("Runtime.evaluate", json!({
        "expression": "JSON.stringify({width: window.innerWidth, height: window.innerHeight})",
        "returnByValue": true,
    })).await?;

    let size_str = result
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_str())
        .unwrap_or(r#"{"width":1800,"height":1200}"#);

    let size: Value =
        serde_json::from_str(size_str).unwrap_or(json!({"width": 1800, "height": 1200}));

    Ok(Json(json!({
        "value": {
            "x": 0,
            "y": 0,
            "width": size.get("width").and_then(|v| v.as_i64()).unwrap_or(1800),
            "height": size.get("height").and_then(|v| v.as_i64()).unwrap_or(1200),
        }
    })))
}

/// GET /session/:id/window/rect — Get window rect
pub async fn get_window_rect(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    query_window_size(&store, &session_id).await
}

/// POST /session/:id/window/maximize
pub async fn maximize_window(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    // In headless mode, just return the current size
    query_window_size(&store, &session_id).await
}

/// POST /session/:id/window/fullscreen
pub async fn fullscreen_window(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    query_window_size(&store, &session_id).await
}

/// POST /session/:id/window/minimize
pub async fn minimize_window(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    query_window_size(&store, &session_id).await
}

/// POST /session/:id/frame — Switch to frame
pub async fn switch_to_frame(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, WebDriverError> {
    let _session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let id = body.get("id");

    match id {
        None | Some(Value::Null) => {
            // Switch to top-level frame — Runtime.evaluate always targets the main frame by default
            Ok(Json(json!({ "value": null })))
        }
        Some(Value::Number(_)) => {
            // Switch to frame by index — no-op, frame context tracking not yet implemented
            Ok(Json(json!({ "value": null })))
        }
        Some(obj) if obj.get("element-6066-11e4-a52e-4f735466cecf").is_some() => {
            // Switch to frame by element — no-op
            Ok(Json(json!({ "value": null })))
        }
        _ => Err(WebDriverError::NoSuchFrame),
    }
}

/// POST /session/:id/frame/parent — Switch to parent frame
pub async fn switch_to_parent_frame(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    let _session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    Ok(Json(json!({ "value": null })))
}
