use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};

use crate::error::WebDriverError;
use crate::session_store::SessionStore;

/// GET /session/:id/element/:element_id/text
pub async fn get_element_text(
    State(store): State<SessionStore>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let result = session
        .call_function_on(
            &element_id,
            "function() { return this.innerText || this.textContent || ''; }",
            vec![],
            true,
        )
        .await?;

    let text = extract_str_result(&result);
    Ok(Json(json!({ "value": text })))
}

/// GET /session/:id/element/:element_id/name — Tag name
pub async fn get_element_tag_name(
    State(store): State<SessionStore>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let result = session
        .call_function_on(
            &element_id,
            "function() { return this.tagName.toLowerCase(); }",
            vec![],
            true,
        )
        .await?;

    let name = extract_str_result(&result);
    Ok(Json(json!({ "value": name })))
}

/// GET /session/:id/element/:element_id/attribute/:name
pub async fn get_element_attribute(
    State(store): State<SessionStore>,
    Path((session_id, element_id, attr_name)): Path<(String, String, String)>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let result = session
        .call_function_on(
            &element_id,
            "function(name) { return this.getAttribute(name); }",
            vec![Value::String(attr_name)],
            true,
        )
        .await?;

    let value = result
        .get("result")
        .and_then(|r| r.get("value"))
        .cloned()
        .unwrap_or(Value::Null);
    Ok(Json(json!({ "value": value })))
}

/// GET /session/:id/element/:element_id/property/:name
pub async fn get_element_property(
    State(store): State<SessionStore>,
    Path((session_id, element_id, prop_name)): Path<(String, String, String)>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let result = session
        .call_function_on(
            &element_id,
            "function(name) { return this[name]; }",
            vec![Value::String(prop_name)],
            true,
        )
        .await?;

    let value = result
        .get("result")
        .and_then(|r| r.get("value"))
        .cloned()
        .unwrap_or(Value::Null);
    Ok(Json(json!({ "value": value })))
}

/// GET /session/:id/element/:element_id/css/:property_name
pub async fn get_element_css(
    State(store): State<SessionStore>,
    Path((session_id, element_id, prop_name)): Path<(String, String, String)>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let result = session
        .call_function_on(
            &element_id,
            "function(prop) { return window.getComputedStyle(this).getPropertyValue(prop); }",
            vec![Value::String(prop_name)],
            true,
        )
        .await?;

    let value = extract_str_result(&result);
    Ok(Json(json!({ "value": value })))
}

/// GET /session/:id/element/:element_id/rect
pub async fn get_element_rect(
    State(store): State<SessionStore>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let result = session.call_function_on(
        &element_id,
        "function() { const r = this.getBoundingClientRect(); return {x: r.x, y: r.y, width: r.width, height: r.height}; }",
        vec![],
        true,
    ).await?;

    let rect = result
        .get("result")
        .and_then(|r| r.get("value"))
        .cloned()
        .unwrap_or(json!({"x": 0, "y": 0, "width": 0, "height": 0}));
    Ok(Json(json!({ "value": rect })))
}

/// GET /session/:id/element/:element_id/enabled
pub async fn is_element_enabled(
    State(store): State<SessionStore>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let result = session
        .call_function_on(
            &element_id,
            "function() { return !this.disabled; }",
            vec![],
            true,
        )
        .await?;

    let enabled = extract_bool_result(&result, true);
    Ok(Json(json!({ "value": enabled })))
}

/// GET /session/:id/element/:element_id/selected
pub async fn is_element_selected(
    State(store): State<SessionStore>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let result = session.call_function_on(
        &element_id,
        "function() { if (this.tagName === 'OPTION') return this.selected; return this.checked || false; }",
        vec![],
        true,
    ).await?;

    let selected = extract_bool_result(&result, false);
    Ok(Json(json!({ "value": selected })))
}

/// GET /session/:id/element/:element_id/displayed
pub async fn is_element_displayed(
    State(store): State<SessionStore>,
    Path((session_id, element_id)): Path<(String, String)>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let result = session.call_function_on(
        &element_id,
        "function() { \
            const s = window.getComputedStyle(this); \
            if (s.display === 'none' || s.visibility === 'hidden' || s.opacity === '0') return false; \
            const r = this.getBoundingClientRect(); \
            return r.width > 0 && r.height > 0; \
        }",
        vec![],
        true,
    ).await?;

    let displayed = extract_bool_result(&result, false);
    Ok(Json(json!({ "value": displayed })))
}

fn extract_str_result(result: &Value) -> &str {
    result
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

fn extract_bool_result(result: &Value, default: bool) -> bool {
    result
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}
