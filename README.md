# webdriver-cdp

Lightweight W3C WebDriver server that proxies to Chrome via CDP. Drop-in replacement for Selenium standalone.

## Quick start (Docker)

```bash
docker run -p 4444:4444 --shm-size=2g ghcr.io/osso/webdriver-cdp:latest
```

## Docker Compose

```yaml
selenium:
  image: ghcr.io/osso/webdriver-cdp:latest
  ports:
    - "4444:4444"
  shm_size: 2g
  extra_hosts:
    - "host.docker.internal:host-gateway"
```

## Native browser forwarding

Run tests in a visible Chrome on your host instead of headless in Docker:

```bash
# Install
cargo install --path .

# Connect to the running container
webdriver-cdp connect

# Options
webdriver-cdp connect --server http://localhost:4444 --port 9222
```

This launches Chrome visibly, starts a TCP proxy, and tells the container to route sessions through it. Press Ctrl+C to disconnect and revert to headless.

## Environment variables

| Variable | Default | Description |
|---|---|---|
| `PORT` | `4444` | WebDriver server port |
| `CHROME_DEBUG_PORT` | `9222` | Chrome CDP port |
| `CHROME_BIN` | auto-detect | Chrome binary path |
| `RUST_LOG` | `webdriver_cdp=info` | Log level |

## Build from source

```bash
cargo build --release
```

## Docker build

```bash
docker build -t webdriver-cdp .
```

Requires 2+ vCPU / 4GB RAM for Rust compilation.
