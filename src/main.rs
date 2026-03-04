mod cdp;
mod chrome;
mod error;
mod handlers;
mod session;
mod session_store;

use axum::{
    Router,
    routing::{delete, get, post},
};
use session_store::SessionStore;
use std::net::SocketAddr;
use std::process::Child;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 && args[1] == "connect" {
        run_connect(&args[2..]).await;
        return;
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "webdriver_cdp=info".parse().unwrap()),
        )
        .init();

    run_server().await;
}

async fn run_server() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(4444);

    let chrome_port: u16 = std::env::var("CHROME_DEBUG_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(9222);

    let chrome = chrome::Chrome::launch(chrome_port).expect("Failed to launch Chrome");
    chrome.wait_ready().await.expect("Chrome CDP not ready");
    let _chrome = Arc::new(chrome);

    let store = SessionStore::new(chrome_port);
    let app = build_router(store);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("WebDriver CDP server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

// --- connect subcommand ---

struct ConnectArgs {
    server: String,
    debug_port: u16,
}

fn parse_connect_args(args: &[String]) -> ConnectArgs {
    let mut server = "http://localhost:4444".to_string();
    let mut debug_port: u16 = 9222;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--server" if i + 1 < args.len() => {
                i += 1;
                server = args[i].clone();
            }
            "--port" if i + 1 < args.len() => {
                i += 1;
                debug_port = args[i].parse().expect("Invalid port number");
            }
            other => {
                eprintln!("Unknown argument: {}", other);
                std::process::exit(1);
            }
        }
        i += 1;
    }
    ConnectArgs { server, debug_port }
}

fn launch_visible_chrome(debug_port: u16) -> Child {
    let bin = std::env::var("CHROME_BIN").unwrap_or_else(|_| "google-chrome-stable".to_string());
    eprintln!("Launching Chrome from {} on port {}...", bin, debug_port);
    std::process::Command::new(&bin)
        .args([
            &format!("--remote-debugging-port={}", debug_port),
            "--window-size=1800,1200",
            "--ignore-certificate-errors",
            "--no-first-run",
            "--disable-background-networking",
            "about:blank",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap_or_else(|e| {
            eprintln!("Failed to launch Chrome: {}", e);
            std::process::exit(1);
        })
}

async fn wait_for_cdp(debug_port: u16) -> bool {
    let url = format!("http://127.0.0.1:{}/json/version", debug_port);
    for _ in 0..50 {
        if reqwest::get(&url).await.is_ok() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    false
}

async fn register_external_chrome(server: &str, debug_port: u16) -> Result<(), String> {
    let url = format!("{}/_admin/chrome", server);
    let body = serde_json::json!({ "url": format!("http://host.docker.internal:{}", debug_port) });
    let resp = reqwest::Client::new()
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to server: {}", e))?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("Server returned {}", resp.status()))
    }
}

async fn run_connect(args: &[String]) {
    let opts = parse_connect_args(args);
    let mut chrome = launch_visible_chrome(opts.debug_port);

    if !wait_for_cdp(opts.debug_port).await {
        eprintln!("Chrome CDP not ready after 5s");
        let _ = chrome.kill();
        std::process::exit(1);
    }
    eprintln!("Chrome CDP ready");

    if let Err(e) = register_external_chrome(&opts.server, opts.debug_port).await {
        eprintln!("{}", e);
        let _ = chrome.kill();
        std::process::exit(1);
    }
    eprintln!(
        "Connected to {} — sessions now use host Chrome",
        opts.server
    );
    eprintln!("Press Ctrl+C to disconnect and close Chrome");

    tokio::signal::ctrl_c().await.ok();
    disconnect_and_cleanup(&opts.server, &mut chrome).await;
}

async fn disconnect_and_cleanup(server: &str, chrome: &mut Child) {
    eprintln!("\nDisconnecting...");
    let url = format!("{}/_admin/chrome", server);
    let _ = reqwest::Client::new().delete(&url).send().await;
    let _ = chrome.kill();
    let _ = chrome.wait();
    eprintln!("Done");
}

// --- router ---

fn build_router(store: SessionStore) -> Router {
    Router::new()
        .merge(status_routes())
        .merge(session_routes())
        .merge(navigation_routes())
        .merge(element_routes())
        .merge(element_info_routes())
        .merge(element_action_routes())
        .merge(js_routes())
        .merge(cookie_routes())
        .merge(window_routes())
        .merge(timeout_routes())
        .merge(screenshot_routes())
        .merge(alert_routes())
        .merge(action_routes())
        .merge(admin_routes())
        .with_state(store)
}

fn status_routes() -> Router<SessionStore> {
    Router::new().route("/status", get(handlers::status::get_status))
}

fn session_routes() -> Router<SessionStore> {
    Router::new()
        .route("/session", post(handlers::session::new_session))
        .route(
            "/session/{session_id}",
            delete(handlers::session::delete_session),
        )
}

fn navigation_routes() -> Router<SessionStore> {
    Router::new()
        .route(
            "/session/{session_id}/url",
            post(handlers::navigation::navigate),
        )
        .route(
            "/session/{session_id}/url",
            get(handlers::navigation::get_url),
        )
        .route(
            "/session/{session_id}/title",
            get(handlers::navigation::get_title),
        )
        .route(
            "/session/{session_id}/back",
            post(handlers::navigation::back),
        )
        .route(
            "/session/{session_id}/forward",
            post(handlers::navigation::forward),
        )
        .route(
            "/session/{session_id}/refresh",
            post(handlers::navigation::refresh),
        )
        .route(
            "/session/{session_id}/source",
            get(handlers::navigation::get_source),
        )
}

fn element_routes() -> Router<SessionStore> {
    Router::new()
        .route(
            "/session/{session_id}/element",
            post(handlers::elements::find_element),
        )
        .route(
            "/session/{session_id}/elements",
            post(handlers::elements::find_elements),
        )
        .route(
            "/session/{session_id}/element/active",
            get(handlers::elements::get_active_element),
        )
        .route(
            "/session/{session_id}/element/{element_id}/element",
            post(handlers::elements::find_child_element),
        )
        .route(
            "/session/{session_id}/element/{element_id}/elements",
            post(handlers::elements::find_child_elements),
        )
}

fn element_info_routes() -> Router<SessionStore> {
    Router::new()
        .route(
            "/session/{session_id}/element/{element_id}/text",
            get(handlers::element_info::get_element_text),
        )
        .route(
            "/session/{session_id}/element/{element_id}/name",
            get(handlers::element_info::get_element_tag_name),
        )
        .route(
            "/session/{session_id}/element/{element_id}/attribute/{attr_name}",
            get(handlers::element_info::get_element_attribute),
        )
        .route(
            "/session/{session_id}/element/{element_id}/property/{prop_name}",
            get(handlers::element_info::get_element_property),
        )
        .route(
            "/session/{session_id}/element/{element_id}/css/{prop_name}",
            get(handlers::element_info::get_element_css),
        )
        .route(
            "/session/{session_id}/element/{element_id}/rect",
            get(handlers::element_info::get_element_rect),
        )
        .route(
            "/session/{session_id}/element/{element_id}/enabled",
            get(handlers::element_info::is_element_enabled),
        )
        .route(
            "/session/{session_id}/element/{element_id}/selected",
            get(handlers::element_info::is_element_selected),
        )
        .route(
            "/session/{session_id}/element/{element_id}/displayed",
            get(handlers::element_info::is_element_displayed),
        )
}

fn element_action_routes() -> Router<SessionStore> {
    Router::new()
        .route(
            "/session/{session_id}/element/{element_id}/click",
            post(handlers::element_actions::element_click),
        )
        .route(
            "/session/{session_id}/element/{element_id}/value",
            post(handlers::element_actions::element_send_keys),
        )
        .route(
            "/session/{session_id}/element/{element_id}/clear",
            post(handlers::element_actions::element_clear),
        )
}

fn js_routes() -> Router<SessionStore> {
    Router::new()
        .route(
            "/session/{session_id}/execute/sync",
            post(handlers::js::execute_sync),
        )
        .route(
            "/session/{session_id}/execute/async",
            post(handlers::js::execute_async),
        )
}

fn cookie_routes() -> Router<SessionStore> {
    Router::new()
        .route(
            "/session/{session_id}/cookie",
            get(handlers::cookies::get_all_cookies),
        )
        .route(
            "/session/{session_id}/cookie",
            post(handlers::cookies::add_cookie),
        )
        .route(
            "/session/{session_id}/cookie",
            delete(handlers::cookies::delete_all_cookies),
        )
        .route(
            "/session/{session_id}/cookie/{name}",
            get(handlers::cookies::get_named_cookie),
        )
        .route(
            "/session/{session_id}/cookie/{name}",
            delete(handlers::cookies::delete_cookie),
        )
}

fn window_routes() -> Router<SessionStore> {
    Router::new()
        .route(
            "/session/{session_id}/window",
            get(handlers::window::get_window_handle),
        )
        .route(
            "/session/{session_id}/window",
            post(handlers::window::switch_to_window),
        )
        .route(
            "/session/{session_id}/window",
            delete(handlers::window::close_window),
        )
        .route(
            "/session/{session_id}/window/handles",
            get(handlers::window::get_window_handles),
        )
        .route(
            "/session/{session_id}/window/rect",
            get(handlers::window::get_window_rect),
        )
        .route(
            "/session/{session_id}/window/rect",
            post(handlers::window::set_window_rect),
        )
        .route(
            "/session/{session_id}/window/maximize",
            post(handlers::window::maximize_window),
        )
        .route(
            "/session/{session_id}/window/fullscreen",
            post(handlers::window::fullscreen_window),
        )
        .route(
            "/session/{session_id}/window/minimize",
            post(handlers::window::minimize_window),
        )
        .route(
            "/session/{session_id}/frame",
            post(handlers::window::switch_to_frame),
        )
        .route(
            "/session/{session_id}/frame/parent",
            post(handlers::window::switch_to_parent_frame),
        )
}

fn timeout_routes() -> Router<SessionStore> {
    Router::new()
        .route(
            "/session/{session_id}/timeouts",
            get(handlers::timeouts::get_timeouts),
        )
        .route(
            "/session/{session_id}/timeouts",
            post(handlers::timeouts::set_timeouts),
        )
}

fn screenshot_routes() -> Router<SessionStore> {
    Router::new()
        .route(
            "/session/{session_id}/screenshot",
            get(handlers::screenshots::take_screenshot),
        )
        .route(
            "/session/{session_id}/element/{element_id}/screenshot",
            get(handlers::screenshots::take_element_screenshot),
        )
}

fn alert_routes() -> Router<SessionStore> {
    Router::new()
        .route(
            "/session/{session_id}/alert/accept",
            post(handlers::alerts::accept_alert),
        )
        .route(
            "/session/{session_id}/alert/dismiss",
            post(handlers::alerts::dismiss_alert),
        )
        .route(
            "/session/{session_id}/alert/text",
            get(handlers::alerts::get_alert_text),
        )
        .route(
            "/session/{session_id}/alert/text",
            post(handlers::alerts::send_alert_text),
        )
}

fn action_routes() -> Router<SessionStore> {
    Router::new()
        .route(
            "/session/{session_id}/actions",
            post(handlers::actions::perform_actions),
        )
        .route(
            "/session/{session_id}/actions",
            delete(handlers::actions::release_actions),
        )
}

fn admin_routes() -> Router<SessionStore> {
    Router::new()
        .route("/_admin/chrome", post(handlers::admin::set_external_chrome))
        .route(
            "/_admin/chrome",
            delete(handlers::admin::clear_external_chrome),
        )
        .route("/_admin/chrome", get(handlers::admin::get_chrome_mode))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("Ctrl+C received"),
        _ = terminate => tracing::info!("SIGTERM received"),
    }
    tracing::info!("Shutting down...");
}
