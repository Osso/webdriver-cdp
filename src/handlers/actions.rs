use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};

use crate::cdp::CdpSession;
use crate::error::WebDriverError;
use crate::session_store::SessionStore;

const ELEMENT_KEY: &str = "element-6066-11e4-a52e-4f735466cecf";

/// POST /session/:id/actions — Perform actions
pub async fn perform_actions(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let action_lists = body
        .get("actions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| WebDriverError::InvalidArgument("Missing 'actions' parameter".into()))?;

    for source in action_lists {
        let source_type = source.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let actions = source.get("actions").and_then(|v| v.as_array());

        if let Some(actions) = actions {
            for action in actions {
                dispatch_action(&session.cdp, source_type, action).await?;
            }
        }
    }

    Ok(Json(json!({ "value": null })))
}

/// DELETE /session/:id/actions — Release actions
pub async fn release_actions(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    let _session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    // Release all pressed keys/buttons — no-op for now
    Ok(Json(json!({ "value": null })))
}

async fn dispatch_action(
    cdp: &CdpSession,
    source_type: &str,
    action: &Value,
) -> Result<(), WebDriverError> {
    match source_type {
        "pointer" => handle_pointer_action(cdp, action).await,
        "key" => handle_key_action(cdp, action).await,
        "none" => handle_pause_action(action).await,
        _ => Ok(()),
    }
}

async fn resolve_pointer_move_coords(
    cdp: &CdpSession,
    action: &Value,
    offset_x: f64,
    offset_y: f64,
) -> Result<(f64, f64), WebDriverError> {
    if let Some(origin) = action.get("origin") {
        if let Some(element_id) = origin.get(ELEMENT_KEY).and_then(|v| v.as_str()) {
            let result = cdp
                .send_command(
                    "Runtime.callFunctionOn",
                    json!({
                        "objectId": element_id,
                        "functionDeclaration": "function() { \
                            const r = this.getBoundingClientRect(); \
                            return {x: r.x + r.width/2, y: r.y + r.height/2}; \
                        }",
                        "returnByValue": true,
                    }),
                )
                .await?;

            let center = result.get("result").and_then(|r| r.get("value"));
            let cx = center
                .and_then(|c| c.get("x"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let cy = center
                .and_then(|c| c.get("y"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            return Ok((cx + offset_x, cy + offset_y));
        }
    }
    Ok((offset_x, offset_y))
}

fn pointer_button_name(button: i64) -> &'static str {
    match button {
        1 => "middle",
        2 => "right",
        _ => "left",
    }
}

async fn handle_pointer_action(cdp: &CdpSession, action: &Value) -> Result<(), WebDriverError> {
    let action_type = action.get("type").and_then(|v| v.as_str()).unwrap_or("");

    match action_type {
        "pointerMove" => {
            let x = action.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let y = action.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let (final_x, final_y) = resolve_pointer_move_coords(cdp, action, x, y).await?;
            cdp.send_command(
                "Input.dispatchMouseEvent",
                json!({
                    "type": "mouseMoved", "x": final_x, "y": final_y,
                }),
            )
            .await?;
        }
        "pointerDown" => {
            let btn =
                pointer_button_name(action.get("button").and_then(|v| v.as_i64()).unwrap_or(0));
            cdp.send_command(
                "Input.dispatchMouseEvent",
                json!({
                    "type": "mousePressed", "x": 0, "y": 0, "button": btn, "clickCount": 1,
                }),
            )
            .await?;
        }
        "pointerUp" => {
            let btn =
                pointer_button_name(action.get("button").and_then(|v| v.as_i64()).unwrap_or(0));
            cdp.send_command(
                "Input.dispatchMouseEvent",
                json!({
                    "type": "mouseReleased", "x": 0, "y": 0, "button": btn, "clickCount": 1,
                }),
            )
            .await?;
        }
        "pause" => sleep_action(action).await,
        _ => {}
    }
    Ok(())
}

async fn handle_key_action(cdp: &CdpSession, action: &Value) -> Result<(), WebDriverError> {
    let action_type = action.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let key_value = action.get("value").and_then(|v| v.as_str()).unwrap_or("");

    match action_type {
        "keyDown" => {
            let key = normalize_key(key_value);
            let text = if key.len() == 1 {
                key.clone()
            } else {
                String::new()
            };
            cdp.send_command(
                "Input.dispatchKeyEvent",
                json!({
                    "type": "keyDown", "key": key, "text": text,
                }),
            )
            .await?;
        }
        "keyUp" => {
            let key = normalize_key(key_value);
            cdp.send_command(
                "Input.dispatchKeyEvent",
                json!({
                    "type": "keyUp", "key": key,
                }),
            )
            .await?;
        }
        "pause" => sleep_action(action).await,
        _ => {}
    }
    Ok(())
}

async fn handle_pause_action(action: &Value) -> Result<(), WebDriverError> {
    sleep_action(action).await;
    Ok(())
}

async fn sleep_action(action: &Value) {
    if let Some(duration) = action.get("duration").and_then(|v| v.as_u64()) {
        tokio::time::sleep(std::time::Duration::from_millis(duration)).await;
    }
}

fn normalize_key(key_value: &str) -> String {
    if let Some(ch) = key_value.chars().next() {
        match ch as u32 {
            0xE003 => "Backspace".to_string(),
            0xE004 => "Tab".to_string(),
            0xE006 | 0xE007 => "Enter".to_string(),
            0xE008 => "Shift".to_string(),
            0xE009 => "Control".to_string(),
            0xE00A => "Alt".to_string(),
            0xE00C => "Escape".to_string(),
            0xE00D => " ".to_string(),
            0xE012 => "ArrowLeft".to_string(),
            0xE013 => "ArrowUp".to_string(),
            0xE014 => "ArrowRight".to_string(),
            0xE015 => "ArrowDown".to_string(),
            _ => key_value.to_string(),
        }
    } else {
        key_value.to_string()
    }
}
