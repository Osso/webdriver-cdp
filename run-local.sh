#!/usr/bin/env bash
# Run webdriver-cdp locally with visible Chrome window.
# Usage: ./run-local.sh [--headless]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ "${1:-}" == "--headless" ]]; then
    export HEADLESS=true
else
    export HEADLESS=false
fi

export RUST_LOG="${RUST_LOG:-webdriver_cdp=info}"

echo "Building webdriver-cdp..."
cargo build --release --manifest-path "${SCRIPT_DIR}/Cargo.toml"

echo "Starting on :4444 (headless=${HEADLESS})"
exec "${SCRIPT_DIR}/target/release/webdriver-cdp"
