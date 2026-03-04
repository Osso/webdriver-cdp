use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};

use crate::error::WebDriverError;
use crate::session_store::SessionStore;

/// POST /session/:id/execute/sync — Execute synchronous JavaScript
pub async fn execute_sync(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let (script, args) = extract_script_and_args(&body)?;
    let wrapped = wrap_sync_script(script, &args);

    let result = session
        .cdp
        .send_command(
            "Runtime.evaluate",
            json!({
                "expression": wrapped,
                "returnByValue": true,
                "awaitPromise": false,
            }),
        )
        .await?;

    let value = extract_result_or_error(&result)?;
    Ok(Json(json!({ "value": value })))
}

/// POST /session/:id/execute/async — Execute asynchronous JavaScript
pub async fn execute_async(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .get(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    let (script, args) = extract_script_and_args(&body)?;
    let timeout_ms = session.script_timeout_ms;
    let wrapped = wrap_async_script(script, &args, timeout_ms);

    let result = session
        .cdp
        .send_command(
            "Runtime.evaluate",
            json!({
                "expression": wrapped,
                "returnByValue": true,
                "awaitPromise": true,
            }),
        )
        .await?;

    let value = extract_result_or_error(&result)?;
    Ok(Json(json!({ "value": value })))
}

fn extract_script_and_args(body: &Value) -> Result<(&str, Vec<Value>), WebDriverError> {
    let script = body
        .get("script")
        .and_then(|v| v.as_str())
        .ok_or_else(|| WebDriverError::InvalidArgument("Missing 'script' parameter".into()))?;
    let args = body
        .get("args")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    Ok((script, args))
}

fn wrap_sync_script(script: &str, args: &[Value]) -> String {
    let args_json = serialize_args(args);
    format!(
        "(() => {{ const __args = {}; return (function() {{ {} }}).apply(null, __args); }})()",
        args_json, script
    )
}

fn wrap_async_script(script: &str, args: &[Value], timeout_ms: u64) -> String {
    let args_json = serialize_args(args);
    format!(
        "new Promise((resolve, reject) => {{ \
            const __args = {}; \
            __args.push(resolve); \
            setTimeout(() => reject(new Error('Script timeout')), {}); \
            (function() {{ {} }}).apply(null, __args); \
        }})",
        args_json, timeout_ms, script
    )
}

fn extract_result_or_error(result: &Value) -> Result<Value, WebDriverError> {
    if let Some(exception) = result.get("exceptionDetails") {
        let msg = exception
            .get("text")
            .or_else(|| {
                exception
                    .get("exception")
                    .and_then(|e| e.get("description"))
            })
            .and_then(|v| v.as_str())
            .unwrap_or("JavaScript error");
        return Err(WebDriverError::JavascriptError(msg.to_string()));
    }

    Ok(result
        .get("result")
        .and_then(|r| r.get("value"))
        .cloned()
        .unwrap_or(Value::Null))
}

fn serialize_args(args: &[Value]) -> String {
    serde_json::to_string(args).unwrap_or_else(|_| "[]".to_string())
}
