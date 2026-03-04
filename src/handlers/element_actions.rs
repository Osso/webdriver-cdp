use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};

use crate::cdp::CdpSession;
use crate::error::WebDriverError;
use crate::session_store::SessionStore;

/// POST /session/:id/element/:element_id/click
pub async fn element_click(
    State(store): State<SessionStore>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let coords = scroll_into_view_and_get_center(&session, &element_id).await?;
    let x = coords.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let y = coords.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);

    dispatch_click(&session.cdp, x, y).await?;
    Ok(Json(json!({ "value": null })))
}

/// POST /session/:id/element/:element_id/value — Send keys
pub async fn element_send_keys(
    State(store): State<SessionStore>,
    Path((session_id, element_id)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let text = extract_keys_text(&body);

    session
        .call_function_on(&element_id, "function() { this.focus(); }", vec![], true)
        .await?;

    for ch in text.chars() {
        dispatch_key_char(&session.cdp, ch).await?;
    }

    Ok(Json(json!({ "value": null })))
}

/// POST /session/:id/element/:element_id/clear
pub async fn element_clear(
    State(store): State<SessionStore>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    session
        .call_function_on(
            &element_id,
            "function() { \
            this.focus(); \
            if (this.tagName === 'INPUT' || this.tagName === 'TEXTAREA') { \
                this.value = ''; \
            } else if (this.isContentEditable) { \
                this.textContent = ''; \
            } \
            this.dispatchEvent(new Event('input', {bubbles: true})); \
            this.dispatchEvent(new Event('change', {bubbles: true})); \
        }",
            vec![],
            true,
        )
        .await?;

    Ok(Json(json!({ "value": null })))
}

async fn scroll_into_view_and_get_center(
    session: &crate::session::Session,
    element_id: &str,
) -> Result<Value, WebDriverError> {
    let result = session
        .call_function_on(
            element_id,
            "function() { \
            this.scrollIntoView({block: 'center', inline: 'center'}); \
            const r = this.getBoundingClientRect(); \
            return {x: r.x + r.width / 2, y: r.y + r.height / 2}; \
        }",
            vec![],
            true,
        )
        .await?;

    result
        .get("result")
        .and_then(|r| r.get("value"))
        .cloned()
        .ok_or(WebDriverError::ElementNotInteractable)
}

async fn dispatch_click(cdp: &CdpSession, x: f64, y: f64) -> Result<(), WebDriverError> {
    cdp.send_command(
        "Input.dispatchMouseEvent",
        json!({
            "type": "mouseMoved", "x": x, "y": y,
        }),
    )
    .await?;
    cdp.send_command(
        "Input.dispatchMouseEvent",
        json!({
            "type": "mousePressed", "x": x, "y": y,
            "button": "left", "clickCount": 1,
        }),
    )
    .await?;
    cdp.send_command(
        "Input.dispatchMouseEvent",
        json!({
            "type": "mouseReleased", "x": x, "y": y,
            "button": "left", "clickCount": 1,
        }),
    )
    .await?;
    Ok(())
}

fn extract_keys_text(body: &Value) -> String {
    if let Some(s) = body.get("text").and_then(|v| v.as_str()) {
        return s.to_string();
    }
    if let Some(arr) = body.get("value").and_then(|v| v.as_array()) {
        return arr
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join("");
    }
    String::new()
}

async fn dispatch_key_char(cdp: &CdpSession, ch: char) -> Result<(), WebDriverError> {
    if let Some(key_info) = special_key(ch) {
        cdp.send_command(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyDown",
                "key": key_info.key,
                "code": key_info.code,
                "windowsVirtualKeyCode": key_info.key_code,
            }),
        )
        .await?;
        cdp.send_command(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyUp",
                "key": key_info.key,
                "code": key_info.code,
                "windowsVirtualKeyCode": key_info.key_code,
            }),
        )
        .await?;
    } else {
        cdp.send_command(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyDown",
                "text": ch.to_string(),
                "key": ch.to_string(),
                "unmodifiedText": ch.to_string(),
            }),
        )
        .await?;
        cdp.send_command(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyUp",
                "key": ch.to_string(),
            }),
        )
        .await?;
    }
    Ok(())
}

struct SpecialKey {
    key: &'static str,
    code: &'static str,
    key_code: u32,
}

fn special_key(ch: char) -> Option<SpecialKey> {
    special_key_navigation(ch).or_else(|| special_key_function(ch))
}

fn special_key_navigation(ch: char) -> Option<SpecialKey> {
    match ch as u32 {
        0xE003 => Some(SpecialKey {
            key: "Backspace",
            code: "Backspace",
            key_code: 8,
        }),
        0xE004 => Some(SpecialKey {
            key: "Tab",
            code: "Tab",
            key_code: 9,
        }),
        0xE006 | 0xE007 => Some(SpecialKey {
            key: "Enter",
            code: "Enter",
            key_code: 13,
        }),
        0xE008 => Some(SpecialKey {
            key: "Shift",
            code: "ShiftLeft",
            key_code: 16,
        }),
        0xE009 => Some(SpecialKey {
            key: "Control",
            code: "ControlLeft",
            key_code: 17,
        }),
        0xE00A => Some(SpecialKey {
            key: "Alt",
            code: "AltLeft",
            key_code: 18,
        }),
        0xE00C => Some(SpecialKey {
            key: "Escape",
            code: "Escape",
            key_code: 27,
        }),
        0xE00D => Some(SpecialKey {
            key: " ",
            code: "Space",
            key_code: 32,
        }),
        0xE00E => Some(SpecialKey {
            key: "PageUp",
            code: "PageUp",
            key_code: 33,
        }),
        0xE00F => Some(SpecialKey {
            key: "PageDown",
            code: "PageDown",
            key_code: 34,
        }),
        0xE010 => Some(SpecialKey {
            key: "End",
            code: "End",
            key_code: 35,
        }),
        0xE011 => Some(SpecialKey {
            key: "Home",
            code: "Home",
            key_code: 36,
        }),
        0xE012 => Some(SpecialKey {
            key: "ArrowLeft",
            code: "ArrowLeft",
            key_code: 37,
        }),
        0xE013 => Some(SpecialKey {
            key: "ArrowUp",
            code: "ArrowUp",
            key_code: 38,
        }),
        0xE014 => Some(SpecialKey {
            key: "ArrowRight",
            code: "ArrowRight",
            key_code: 39,
        }),
        0xE015 => Some(SpecialKey {
            key: "ArrowDown",
            code: "ArrowDown",
            key_code: 40,
        }),
        0xE016 => Some(SpecialKey {
            key: "Insert",
            code: "Insert",
            key_code: 45,
        }),
        0xE017 => Some(SpecialKey {
            key: "Delete",
            code: "Delete",
            key_code: 46,
        }),
        _ => None,
    }
}

fn special_key_function(ch: char) -> Option<SpecialKey> {
    match ch as u32 {
        0xE031 => Some(SpecialKey {
            key: "F1",
            code: "F1",
            key_code: 112,
        }),
        0xE032 => Some(SpecialKey {
            key: "F2",
            code: "F2",
            key_code: 113,
        }),
        0xE033 => Some(SpecialKey {
            key: "F3",
            code: "F3",
            key_code: 114,
        }),
        0xE034 => Some(SpecialKey {
            key: "F4",
            code: "F4",
            key_code: 115,
        }),
        0xE035 => Some(SpecialKey {
            key: "F5",
            code: "F5",
            key_code: 116,
        }),
        0xE036 => Some(SpecialKey {
            key: "F6",
            code: "F6",
            key_code: 117,
        }),
        0xE037 => Some(SpecialKey {
            key: "F7",
            code: "F7",
            key_code: 118,
        }),
        0xE038 => Some(SpecialKey {
            key: "F8",
            code: "F8",
            key_code: 119,
        }),
        0xE039 => Some(SpecialKey {
            key: "F9",
            code: "F9",
            key_code: 120,
        }),
        0xE03A => Some(SpecialKey {
            key: "F10",
            code: "F10",
            key_code: 121,
        }),
        0xE03B => Some(SpecialKey {
            key: "F11",
            code: "F11",
            key_code: 122,
        }),
        0xE03C => Some(SpecialKey {
            key: "F12",
            code: "F12",
            key_code: 123,
        }),
        _ => None,
    }
}
