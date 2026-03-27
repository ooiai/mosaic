#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)

if [ "${MOSAIC_REAL_TESTS:-0}" != "1" ]; then
    echo "real integration tests skipped: set MOSAIC_REAL_TESTS=1 to enable"
    exit 0
fi

cd "$ROOT"

cargo test --manifest-path "$ROOT/Cargo.toml" -p mosaic-provider --test real_vendors -- --nocapture
cargo test --manifest-path "$ROOT/Cargo.toml" -p mosaic-gateway --test real_telegram_ingress -- --nocapture
cargo test --manifest-path "$ROOT/Cargo.toml" -p mosaic-sdk --test real_gateway_http -- --nocapture
cargo test --manifest-path "$ROOT/Cargo.toml" -p mosaic-mcp-core --test real_stdio_mcp -- --nocapture
sh "$ROOT/scripts/test-full-stack-example.sh" openai
