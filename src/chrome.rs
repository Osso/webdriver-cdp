use anyhow::{Context, Result};
use std::process::{Child, Command};
use tracing::info;

pub struct Chrome {
    process: Child,
    debug_port: u16,
}

fn is_headless() -> bool {
    !matches!(
        std::env::var("HEADLESS").as_deref(),
        Ok("0" | "false" | "no")
    )
}

fn chrome_args(debug_port: u16) -> Vec<String> {
    let mut args = Vec::new();
    if is_headless() {
        args.push("--headless=new".into());
    }
    args.extend([
        format!("--remote-debugging-port={}", debug_port),
        "--no-sandbox".into(),
        "--disable-gpu".into(),
        "--disable-dev-shm-usage".into(),
        "--disable-background-networking".into(),
        "--disable-extensions".into(),
        "--disable-sync".into(),
        "--disable-translate".into(),
        "--metrics-recording-only".into(),
        "--no-first-run".into(),
        "--safebrowsing-disable-auto-update".into(),
        "--ignore-certificate-errors".into(),
        "--window-size=1800,1200".into(),
        "about:blank".into(),
    ]);
    args
}

fn find_chrome_binary() -> String {
    let candidates = [
        "google-chrome-stable",
        "google-chrome",
        "chromium-browser",
        "chromium",
        "/usr/bin/google-chrome-stable",
        "/usr/bin/chromium-browser",
        "/usr/bin/chromium",
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
    ];
    for c in candidates {
        if Command::new("which")
            .arg(c)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return c.to_string();
        }
    }
    "google-chrome-stable".to_string()
}

impl Chrome {
    /// Launch Chrome with CDP enabled.
    pub fn launch(debug_port: u16) -> Result<Self> {
        let chrome_bin = std::env::var("CHROME_BIN").unwrap_or_else(|_| find_chrome_binary());

        info!(
            "Launching Chrome from {} on port {}",
            chrome_bin, debug_port
        );

        let process = Command::new(&chrome_bin)
            .args(chrome_args(debug_port))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context(format!("Failed to launch Chrome from {}", chrome_bin))?;

        info!("Chrome launched with PID {}", process.id());
        Ok(Self {
            process,
            debug_port,
        })
    }

    pub fn debug_port(&self) -> u16 {
        self.debug_port
    }

    /// Wait for Chrome's CDP endpoint to be ready.
    pub async fn wait_ready(&self) -> Result<()> {
        let url = format!("http://127.0.0.1:{}/json/version", self.debug_port);
        for i in 0..50 {
            if reqwest::get(&url).await.is_ok() {
                info!("Chrome CDP ready after {}ms", i * 100);
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        anyhow::bail!("Chrome CDP not ready after 5s")
    }
}

impl Drop for Chrome {
    fn drop(&mut self) {
        info!("Killing Chrome process {}", self.process.id());
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

#[derive(serde::Deserialize, Debug)]
pub struct TargetInfo {
    pub id: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub web_socket_debugger_url: Option<String>,
}

/// Create a new browser target via Chrome's HTTP API.
pub async fn create_target(port: u16, url: &str) -> Result<TargetInfo> {
    create_target_on(&format!("http://127.0.0.1:{}", port), url).await
}

/// Create a new browser target on a Chrome instance at the given base URL.
pub async fn create_target_on(base_url: &str, url: &str) -> Result<TargetInfo> {
    let endpoint = format!("{}/json/new?{}", base_url, urlencoding::encode(url));
    let client = reqwest::Client::new();
    let mut target: TargetInfo = client
        .put(&endpoint)
        .send()
        .await
        .context("Failed to create target")?
        .json()
        .await
        .context("Failed to parse target info")?;

    // Chrome returns WS URLs with its own hostname (e.g. ws://localhost:9222/...).
    // When connecting from a container to host Chrome, we must rewrite to match base_url.
    if let Some(ws) = &target.web_socket_debugger_url {
        if let Some(host_port) = base_url.strip_prefix("http://") {
            let rewritten = rewrite_ws_host(ws, host_port);
            target.web_socket_debugger_url = Some(rewritten);
        }
    }
    Ok(target)
}

/// Rewrite the host:port in a ws:// URL to match the given host_port.
fn rewrite_ws_host(ws_url: &str, host_port: &str) -> String {
    if let Some(path) = ws_url.strip_prefix("ws://") {
        if let Some(slash) = path.find('/') {
            return format!("ws://{}{}", host_port, &path[slash..]);
        }
    }
    ws_url.to_string()
}

/// Close a browser target via Chrome's HTTP API.
pub async fn close_target(port: u16, target_id: &str) -> Result<()> {
    close_target_on(&format!("http://127.0.0.1:{}", port), target_id).await
}

/// Close a browser target on a Chrome instance at the given base URL.
pub async fn close_target_on(base_url: &str, target_id: &str) -> Result<()> {
    let endpoint = format!("{}/json/close/{}", base_url, target_id);
    reqwest::Client::new()
        .put(&endpoint)
        .send()
        .await
        .context("Failed to close target")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_ws_host_replaces_localhost() {
        let result = rewrite_ws_host(
            "ws://localhost:9222/devtools/page/ABC",
            "host.docker.internal:9222",
        );
        assert_eq!(result, "ws://host.docker.internal:9222/devtools/page/ABC");
    }

    #[test]
    fn rewrite_ws_host_replaces_ip() {
        let result = rewrite_ws_host(
            "ws://127.0.0.1:9222/devtools/page/XYZ",
            "host.docker.internal:5555",
        );
        assert_eq!(result, "ws://host.docker.internal:5555/devtools/page/XYZ");
    }

    #[test]
    fn rewrite_ws_host_preserves_non_ws() {
        let result = rewrite_ws_host("http://localhost:9222/foo", "other:1234");
        assert_eq!(result, "http://localhost:9222/foo");
    }

    #[test]
    fn rewrite_ws_host_no_path() {
        let result = rewrite_ws_host("ws://localhost:9222", "other:1234");
        assert_eq!(result, "ws://localhost:9222");
    }

    #[test]
    fn chrome_args_includes_headless_by_default() {
        let args = chrome_args(9222);
        assert!(args.contains(&"--headless=new".to_string()));
        assert!(args.contains(&"--remote-debugging-port=9222".to_string()));
        assert!(args.contains(&"--no-sandbox".to_string()));
        assert!(args.contains(&"--ignore-certificate-errors".to_string()));
    }

    #[test]
    fn chrome_args_includes_debug_port() {
        let args = chrome_args(5555);
        assert!(args.contains(&"--remote-debugging-port=5555".to_string()));
    }
}
