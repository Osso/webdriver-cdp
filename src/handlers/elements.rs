use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};

use crate::error::WebDriverError;
use crate::session_store::SessionStore;

pub const ELEMENT_KEY: &str = "element-6066-11e4-a52e-4f735466cecf";

/// POST /session/:id/element — Find single element
pub async fn find_element(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let (using, value) = parse_locator(&body)?;
    let js = build_find_js(&using, &value, false);

    let result = session
        .cdp
        .send_command(
            "Runtime.evaluate",
            json!({
                "expression": js,
                "returnByValue": false,
            }),
        )
        .await?;

    let object_id = extract_object_id(&result)?;
    Ok(Json(json!({ "value": { ELEMENT_KEY: object_id } })))
}

/// POST /session/:id/elements — Find multiple elements
pub async fn find_elements(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let (using, value) = parse_locator(&body)?;
    let count_js = build_find_js(&using, &value, true);

    let count_result = session
        .cdp
        .send_command(
            "Runtime.evaluate",
            json!({
                "expression": format!("({}).length", count_js.trim_end_matches(';')),
                "returnByValue": true,
            }),
        )
        .await?;

    let count = count_result
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let elements = collect_indexed_elements(&session, &count_js, count).await?;
    Ok(Json(json!({ "value": elements })))
}

/// POST /session/:id/element/:element_id/element — Find child element
pub async fn find_child_element(
    State(store): State<SessionStore>,
    Path((session_id, element_id)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let (using, value) = parse_locator(&body)?;
    let js = build_child_find_js(&using, &value);

    let result = session
        .call_function_on(&element_id, &js, vec![], false)
        .await?;
    let object_id = extract_object_id(&result)?;
    Ok(Json(json!({ "value": { ELEMENT_KEY: object_id } })))
}

/// POST /session/:id/element/:element_id/elements — Find child elements
pub async fn find_child_elements(
    State(store): State<SessionStore>,
    Path((session_id, element_id)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let (using, value) = parse_locator(&body)?;

    let count_result = session
        .call_function_on(
            &element_id,
            &build_child_count_js(&using, &value),
            vec![],
            true,
        )
        .await?;

    let count = count_result
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let mut elements = Vec::new();
    for i in 0..count {
        let child_js = build_child_nth_js(&using, &value, i);
        let result = session
            .call_function_on(&element_id, &child_js, vec![], false)
            .await?;
        if let Ok(oid) = extract_object_id(&result) {
            elements.push(json!({ ELEMENT_KEY: oid }));
        }
    }
    Ok(Json(json!({ "value": elements })))
}

/// GET /session/:id/element/active — Get active (focused) element
pub async fn get_active_element(
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
                "expression": "document.activeElement",
                "returnByValue": false,
            }),
        )
        .await?;

    let object_id = extract_object_id(&result)?;
    Ok(Json(json!({ "value": { ELEMENT_KEY: object_id } })))
}

async fn collect_indexed_elements(
    session: &crate::session::Session,
    collection_js: &str,
    count: u64,
) -> Result<Vec<Value>, WebDriverError> {
    let base = collection_js.trim_end_matches(';');
    let mut elements = Vec::new();
    for i in 0..count {
        let elem_js = format!("({})[{}]", base, i);
        let result = session
            .cdp
            .send_command(
                "Runtime.evaluate",
                json!({
                    "expression": elem_js,
                    "returnByValue": false,
                }),
            )
            .await?;
        if let Ok(oid) = extract_object_id(&result) {
            elements.push(json!({ ELEMENT_KEY: oid }));
        }
    }
    Ok(elements)
}

fn parse_locator(body: &Value) -> Result<(String, String), WebDriverError> {
    let using = body
        .get("using")
        .and_then(|v| v.as_str())
        .ok_or_else(|| WebDriverError::InvalidArgument("Missing 'using' parameter".into()))?
        .to_string();
    let value = body
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or_else(|| WebDriverError::InvalidArgument("Missing 'value' parameter".into()))?
        .to_string();
    Ok((using, value))
}

fn build_css_find_js(escaped: &str, multiple: bool) -> String {
    if multiple {
        format!("document.querySelectorAll({})", escaped)
    } else {
        format!(
            "(() => {{ const el = document.querySelector({}); if (!el) throw new Error('no such element'); return el; }})()",
            escaped
        )
    }
}

fn build_xpath_find_js(escaped: &str, multiple: bool) -> String {
    if multiple {
        format!(
            "(() => {{ const r = document.evaluate({}, document, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null); const a = []; for (let i = 0; i < r.snapshotLength; i++) a.push(r.snapshotItem(i)); return a; }})()",
            escaped
        )
    } else {
        format!(
            "(() => {{ const r = document.evaluate({}, document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null); if (!r.singleNodeValue) throw new Error('no such element'); return r.singleNodeValue; }})()",
            escaped
        )
    }
}

fn build_link_text_find_js(escaped: &str, multiple: bool) -> String {
    if multiple {
        format!(
            "Array.from(document.querySelectorAll('a')).filter(a => a.textContent.trim() === {})",
            escaped
        )
    } else {
        format!(
            "(() => {{ const el = Array.from(document.querySelectorAll('a')).find(a => a.textContent.trim() === {}); if (!el) throw new Error('no such element'); return el; }})()",
            escaped
        )
    }
}

fn build_partial_link_text_find_js(escaped: &str, multiple: bool) -> String {
    if multiple {
        format!(
            "Array.from(document.querySelectorAll('a')).filter(a => a.textContent.includes({}))",
            escaped
        )
    } else {
        format!(
            "(() => {{ const el = Array.from(document.querySelectorAll('a')).find(a => a.textContent.includes({})); if (!el) throw new Error('no such element'); return el; }})()",
            escaped
        )
    }
}

fn build_tag_name_find_js(escaped: &str, multiple: bool) -> String {
    if multiple {
        format!("document.getElementsByTagName({})", escaped)
    } else {
        format!(
            "(() => {{ const el = document.getElementsByTagName({})[0]; if (!el) throw new Error('no such element'); return el; }})()",
            escaped
        )
    }
}

fn build_find_js(using: &str, value: &str, multiple: bool) -> String {
    let escaped = serde_json::to_string(value).unwrap_or_default();
    match using {
        "css selector" => build_css_find_js(&escaped, multiple),
        "xpath" => build_xpath_find_js(&escaped, multiple),
        "link text" => build_link_text_find_js(&escaped, multiple),
        "partial link text" => build_partial_link_text_find_js(&escaped, multiple),
        "tag name" => build_tag_name_find_js(&escaped, multiple),
        _ => format!(
            "(() => {{ throw new Error('Unsupported locator: {}'); }})()",
            using
        ),
    }
}

fn build_child_find_js(using: &str, value: &str) -> String {
    let escaped = serde_json::to_string(value).unwrap_or_default();
    match using {
        "css selector" => format!(
            "function() {{ const el = this.querySelector({}); if (!el) throw new Error('no such element'); return el; }}",
            escaped
        ),
        "xpath" => format!(
            "function() {{ const r = document.evaluate({}, this, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null); if (!r.singleNodeValue) throw new Error('no such element'); return r.singleNodeValue; }}",
            escaped
        ),
        "tag name" => format!(
            "function() {{ const el = this.getElementsByTagName({})[0]; if (!el) throw new Error('no such element'); return el; }}",
            escaped
        ),
        _ => format!(
            "function() {{ throw new Error('Unsupported locator: {}'); }}",
            using
        ),
    }
}

fn build_child_count_js(using: &str, value: &str) -> String {
    let escaped = serde_json::to_string(value).unwrap_or_default();
    match using {
        "css selector" => format!(
            "function() {{ return this.querySelectorAll({}).length; }}",
            escaped
        ),
        "xpath" => format!(
            "function() {{ const r = document.evaluate({}, this, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null); return r.snapshotLength; }}",
            escaped
        ),
        "tag name" => format!(
            "function() {{ return this.getElementsByTagName({}).length; }}",
            escaped
        ),
        _ => "function() { return 0; }".to_string(),
    }
}

fn build_child_nth_js(using: &str, value: &str, index: u64) -> String {
    let escaped = serde_json::to_string(value).unwrap_or_default();
    match using {
        "css selector" => format!(
            "function() {{ return this.querySelectorAll({})[{}]; }}",
            escaped, index
        ),
        "xpath" => format!(
            "function() {{ const r = document.evaluate({}, this, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null); return r.snapshotItem({}); }}",
            escaped, index
        ),
        "tag name" => format!(
            "function() {{ return this.getElementsByTagName({})[{}]; }}",
            escaped, index
        ),
        _ => "function() { return null; }".to_string(),
    }
}

pub fn extract_object_id(result: &Value) -> Result<String, WebDriverError> {
    if let Some(exception) = result.get("exceptionDetails") {
        let text = exception
            .get("text")
            .or_else(|| {
                exception
                    .get("exception")
                    .and_then(|e| e.get("description"))
            })
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown error");
        if text.contains("no such element") {
            return Err(WebDriverError::NoSuchElement);
        }
        return Err(WebDriverError::JavascriptError(text.to_string()));
    }

    result
        .get("result")
        .and_then(|r| r.get("objectId"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or(WebDriverError::NoSuchElement)
}
