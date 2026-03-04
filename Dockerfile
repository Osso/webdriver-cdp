FROM rust:1.92-bookworm AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ src/

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    chromium \
    curl \
    dumb-init \
    && rm -rf /var/lib/apt/lists/*

# Chromium policies (disable DoH etc)
COPY policies/ /etc/chromium/policies/managed/

COPY --from=builder /build/target/release/webdriver-cdp /usr/local/bin/webdriver-cdp

ENV CHROME_BIN=/usr/bin/chromium
ENV PORT=4444
ENV CHROME_DEBUG_PORT=9222
ENV RUST_LOG=webdriver_cdp=info

EXPOSE 4444

ENTRYPOINT ["dumb-init", "--"]
CMD ["webdriver-cdp"]
