use axum::Json;
use axum::extract::{Path, State};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::cdp::CdpSession;
use crate::chrome;
use crate::error::WebDriverError;
use crate::session::Session;
use crate::session_store::SessionStore;

struct Timeouts {
    implicit: Option<u64>,
    page_load: Option<u64>,
    script: Option<u64>,
}

fn extract_timeouts(body: &Value) -> Timeouts {
    let timeouts = body
        .get("capabilities")
        .and_then(|c| c.get("alwaysMatch"))
        .and_then(|a| a.get("timeouts"))
        .or_else(|| {
            body.get("capabilities")
                .and_then(|c| c.get("firstMatch"))
                .and_then(|f| f.get(0))
                .and_then(|m| m.get("timeouts"))
        })
        .or_else(|| {
            body.get("desiredCapabilities")
                .and_then(|d| d.get("timeouts"))
        });

    Timeouts {
        implicit: timeouts
            .and_then(|t| t.get("implicit"))
            .and_then(|v| v.as_u64()),
        page_load: timeouts
            .and_then(|t| t.get("pageLoad"))
            .and_then(|v| v.as_u64()),
        script: timeouts
            .and_then(|t| t.get("script"))
            .and_then(|v| v.as_u64()),
    }
}

async fn enable_cdp_domains(cdp: &CdpSession) -> Result<(), WebDriverError> {
    cdp.send_command("Page.enable", json!({})).await?;
    cdp.send_command("Runtime.enable", json!({})).await?;
    cdp.send_command("Network.enable", json!({})).await?;
    Ok(())
}

fn build_session(
    session_id: String,
    target_id: String,
    cdp: CdpSession,
    timeouts: &Timeouts,
) -> Session {
    let mut session = Session::new(session_id, target_id, cdp);
    if let Some(implicit) = timeouts.implicit {
        session.implicit_wait_ms = implicit;
    }
    if let Some(page_load) = timeouts.page_load {
        session.page_load_timeout_ms = page_load;
    }
    if let Some(script) = timeouts.script {
        session.script_timeout_ms = script;
    }
    session
}

fn session_capabilities_response(session_id: &str, timeouts: &Timeouts) -> Value {
    json!({
        "value": {
            "sessionId": session_id,
            "capabilities": {
                "browserName": "chrome",
                "browserVersion": "",
                "platformName": "linux",
                "acceptInsecureCerts": true,
                "pageLoadStrategy": "none",
                "timeouts": {
                    "implicit": timeouts.implicit.unwrap_or(0),
                    "pageLoad": timeouts.page_load.unwrap_or(300_000),
                    "script": timeouts.script.unwrap_or(30_000)
                },
                "proxy": {},
                "setWindowRect": true,
                "strictFileInteractability": false,
                "unhandledPromptBehavior": "dismiss and notify"
            }
        }
    })
}

/// POST /session — Create new WebDriver session
pub async fn new_session(
    State(store): State<SessionStore>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, WebDriverError> {
    let session_id = Uuid::new_v4().to_string();
    tracing::info!("Creating session {}", session_id);

    let timeouts = extract_timeouts(&body);

    let external = store.external_chrome_url.read().await.clone();
    let target = match &external {
        Some(url) => chrome::create_target_on(url, "about:blank").await,
        None => chrome::create_target(store.chrome_port, "about:blank").await,
    }
    .map_err(|e| WebDriverError::SessionNotCreated(e.to_string()))?;

    let ws_url = target
        .web_socket_debugger_url
        .ok_or_else(|| WebDriverError::SessionNotCreated("No WebSocket URL for target".into()))?;

    let cdp = CdpSession::connect(&ws_url)
        .await
        .map_err(|e| WebDriverError::SessionNotCreated(e.to_string()))?;

    enable_cdp_domains(&cdp).await?;

    let session = build_session(session_id.clone(), target.id, cdp, &timeouts);
    store.sessions.insert(session_id.clone(), session);

    tracing::info!("Session {} created", session_id);
    Ok(Json(session_capabilities_response(&session_id, &timeouts)))
}

/// DELETE /session/:session_id
pub async fn delete_session(
    State(store): State<SessionStore>,
    Path(session_id): Path<String>,
) -> Result<Json<Value>, WebDriverError> {
    let session = store
        .sessions
        .remove(&session_id)
        .ok_or(WebDriverError::InvalidSessionId)?;

    tracing::info!("Deleting session {}", session_id);
    let external = store.external_chrome_url.read().await.clone();
    match &external {
        Some(url) => {
            let _ = chrome::close_target_on(url, &session.1.target_id).await;
        }
        None => {
            let _ = chrome::close_target(store.chrome_port, &session.1.target_id).await;
        }
    }

    Ok(Json(json!({ "value": null })))
}
