use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};

use crate::error::WebDriverError;
use crate::session_store::SessionStore;

/// GET /session/:id/screenshot — Full page screenshot (base64 PNG)
pub async fn take_screenshot(
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
            "Page.captureScreenshot",
            json!({
                "format": "png",
            }),
        )
        .await?;

    let data = result.get("data").and_then(|v| v.as_str()).unwrap_or("");

    Ok(Json(json!({ "value": data })))
}

async fn get_element_bounding_rect(
    session: &crate::session::Session,
    element_id: &str,
) -> Result<Value, WebDriverError> {
    let result = session
        .call_function_on(
            element_id,
            "function() { \
            this.scrollIntoView({block: 'center'}); \
            const r = this.getBoundingClientRect(); \
            return {x: r.x, y: r.y, width: r.width, height: r.height}; \
        }",
            vec![],
            true,
        )
        .await?;

    result
        .get("result")
        .and_then(|r| r.get("value"))
        .cloned()
        .ok_or_else(|| WebDriverError::UnknownError("Failed to get element rect".into()))
}

/// GET /session/:id/element/:element_id/screenshot — Element screenshot
pub async fn take_element_screenshot(
    State(store): State<SessionStore>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let rect = get_element_bounding_rect(&session, &element_id).await?;

    let x = rect.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let y = rect.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let width = rect.get("width").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let height = rect.get("height").and_then(|v| v.as_f64()).unwrap_or(0.0);

    let result = session
        .cdp
        .send_command(
            "Page.captureScreenshot",
            json!({
                "format": "png",
                "clip": { "x": x, "y": y, "width": width, "height": height, "scale": 1 },
            }),
        )
        .await?;

    let data = result.get("data").and_then(|v| v.as_str()).unwrap_or("");

    Ok(Json(json!({ "value": data })))
}
