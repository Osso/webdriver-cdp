use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};

use crate::error::WebDriverError;
use crate::session_store::SessionStore;

/// GET /session/:id/cookie — Get all cookies
pub async fn get_all_cookies(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let result = session
        .cdp
        .send_command("Network.getCookies", json!({}))
        .await?;

    let cookies: Vec<Value> = result
        .get("cookies")
        .and_then(|c| c.as_array())
        .map(|arr| arr.iter().map(cdp_cookie_to_webdriver).collect())
        .unwrap_or_default();

    Ok(Json(json!({ "value": cookies })))
}

/// GET /session/:id/cookie/:name — Get named cookie
pub async fn get_named_cookie(
    State(store): State<SessionStore>,
    Path((session_id, name)): Path<(String, String)>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let result = session
        .cdp
        .send_command("Network.getCookies", json!({}))
        .await?;

    let cookie = result
        .get("cookies")
        .and_then(|c| c.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|c| c.get("name").and_then(|n| n.as_str()) == Some(&name))
        })
        .map(cdp_cookie_to_webdriver)
        .ok_or_else(|| WebDriverError::NoSuchCookie(format!("Cookie '{}' not found", name)))?;

    Ok(Json(json!({ "value": cookie })))
}

fn build_set_cookie_params(cookie: &Value) -> Result<Value, WebDriverError> {
    let name = cookie
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| WebDriverError::InvalidArgument("Missing cookie name".into()))?;
    let value = cookie
        .get("value")
        .and_then(|v| v.as_str())
        .ok_or_else(|| WebDriverError::InvalidArgument("Missing cookie value".into()))?;

    let mut params = json!({ "name": name, "value": value });

    if let Some(domain) = cookie.get("domain").and_then(|v| v.as_str()) {
        params["domain"] = json!(domain);
    }
    if let Some(path) = cookie.get("path").and_then(|v| v.as_str()) {
        params["path"] = json!(path);
    }
    if let Some(secure) = cookie.get("secure").and_then(|v| v.as_bool()) {
        params["secure"] = json!(secure);
    }
    if let Some(http_only) = cookie.get("httpOnly").and_then(|v| v.as_bool()) {
        params["httpOnly"] = json!(http_only);
    }
    if let Some(expiry) = cookie.get("expiry").and_then(|v| v.as_f64()) {
        params["expires"] = json!(expiry);
    }
    if let Some(same_site) = cookie.get("sameSite").and_then(|v| v.as_str()) {
        params["sameSite"] = json!(same_site);
    }

    Ok(params)
}

async fn fill_cookie_url_params(
    cdp: &crate::cdp::CdpSession,
    params: &mut Value,
) -> Result<(), WebDriverError> {
    if params.get("domain").is_none() {
        let r = cdp
            .send_command(
                "Runtime.evaluate",
                json!({
                    "expression": "window.location.hostname",
                    "returnByValue": true,
                }),
            )
            .await?;
        if let Some(host) = r
            .get("result")
            .and_then(|r| r.get("value"))
            .and_then(|v| v.as_str())
        {
            params["domain"] = json!(host);
        }
    }

    let r = cdp
        .send_command(
            "Runtime.evaluate",
            json!({
                "expression": "window.location.href",
                "returnByValue": true,
            }),
        )
        .await?;
    if let Some(url) = r
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_str())
    {
        params["url"] = json!(url);
    }

    Ok(())
}

/// POST /session/:id/cookie — Add cookie
pub async fn add_cookie(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let cookie = body
        .get("cookie")
        .ok_or_else(|| WebDriverError::InvalidArgument("Missing 'cookie' parameter".into()))?;

    let mut params = build_set_cookie_params(cookie)?;
    fill_cookie_url_params(&session.cdp, &mut params).await?;
    session
        .cdp
        .send_command("Network.setCookie", params)
        .await?;

    Ok(Json(json!({ "value": null })))
}

/// DELETE /session/:id/cookie/:name — Delete named cookie
pub async fn delete_cookie(
    State(store): State<SessionStore>,
    Path((session_id, name)): Path<(String, String)>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let url_result = session
        .cdp
        .send_command(
            "Runtime.evaluate",
            json!({
                "expression": "window.location.href",
                "returnByValue": true,
            }),
        )
        .await?;
    let url = url_result
        .get("result")
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    session
        .cdp
        .send_command(
            "Network.deleteCookies",
            json!({
                "name": name,
                "url": url,
            }),
        )
        .await?;

    Ok(Json(json!({ "value": null })))
}

/// DELETE /session/:id/cookie — Delete all cookies
pub async fn delete_all_cookies(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    session
        .cdp
        .send_command("Network.clearBrowserCookies", json!({}))
        .await?;

    Ok(Json(json!({ "value": null })))
}

fn cdp_cookie_to_webdriver(cookie: &Value) -> Value {
    json!({
        "name": cookie.get("name").and_then(|v| v.as_str()).unwrap_or(""),
        "value": cookie.get("value").and_then(|v| v.as_str()).unwrap_or(""),
        "domain": cookie.get("domain").and_then(|v| v.as_str()).unwrap_or(""),
        "path": cookie.get("path").and_then(|v| v.as_str()).unwrap_or("/"),
        "secure": cookie.get("secure").and_then(|v| v.as_bool()).unwrap_or(false),
        "httpOnly": cookie.get("httpOnly").and_then(|v| v.as_bool()).unwrap_or(false),
        "sameSite": cookie.get("sameSite").and_then(|v| v.as_str()).unwrap_or("None"),
        "expiry": cookie.get("expires").and_then(|v| v.as_f64()),
    })
}
