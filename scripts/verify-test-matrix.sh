#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
missing=0

require_file() {
    path=$1
    if [ ! -f "$ROOT/$path" ]; then
        echo "missing matrix artifact: $path" >&2
        missing=1
    fi
}

require_contains() {
    path=$1
    pattern=$2
    if ! grep -Fq "$pattern" "$ROOT/$path"; then
        echo "missing matrix pattern in $path: $pattern" >&2
        missing=1
    fi
}

for path in \
    docs/testing.md \
    docs/real-vs-mock-acceptance.md \
    docs/providers.md \
    docs/release.md \
    docs/telegram-real-e2e.md \
    examples/TESTING.md \
    scripts/test-real-integrations.sh \
    scripts/test-full-stack-example.sh \
    Makefile
do
    require_file "$path"
done

for layer in \
    'unit' \
    'local integration' \
    'protocol-real' \
    'product-real' \
    'release-blocking acceptance'
do
    require_contains "docs/testing.md" "$layer"
done

for crate in \
    'mosaic-config' \
    'mosaic-provider' \
    'mosaic-runtime' \
    'mosaic-tool-core' \
    'mosaic-skill-core' \
    'mosaic-workflow' \
    'mosaic-extension-core' \
    'mosaic-gateway' \
    'mosaic-session-core' \
    'mosaic-inspect' \
    'mosaic-control-protocol' \
    'mosaic-sdk' \
    'mosaic-channel-telegram'
do
    require_contains "docs/testing.md" "$crate"
done

for pattern in \
    'Telegram-first release-blocking acceptance lane' \
    'OpenAI provider-real lane' \
    'operator-manual release-blocking acceptance' \
    'make test-matrix'
do
    require_contains "docs/real-vs-mock-acceptance.md" "$pattern"
done

for pattern in \
    'Vendor Real Proof Lanes' \
    'OpenAI' \
    'Azure OpenAI' \
    'Anthropic' \
    'Ollama'
do
    require_contains "docs/providers.md" "$pattern"
done

for pattern in \
    'mosaic adapter telegram webhook set' \
    'mosaic adapter telegram webhook info' \
    'mosaic adapter telegram test-send' \
    '/mosaic workflow summarize_operator_note'
do
    require_contains "docs/telegram-real-e2e.md" "$pattern"
done

for pattern in \
    'make test-matrix' \
    'MOSAIC_REAL_TESTS=1 make test-real' \
    'telegram-real-e2e.md' \
    'Compatibility addendum lanes'
do
    require_contains "docs/release.md" "$pattern"
done

for pattern in \
    'make test-matrix' \
    'docs/telegram-real-e2e.md' \
    'openai-webchat'
do
    require_contains "examples/TESTING.md" "$pattern"
done

for pattern in \
    'mosaic-provider' \
    'mosaic-gateway' \
    'mosaic-sdk' \
    'mosaic-mcp-core' \
    'openai-webchat'
do
    require_contains "scripts/test-real-integrations.sh" "$pattern"
done

for pattern in \
    'openai-webchat' \
    'mock-telegram'
do
    require_contains "scripts/test-full-stack-example.sh" "$pattern"
done

for pattern in \
    'test-matrix:' \
    '$(MAKE) test-matrix'
do
    require_contains "Makefile" "$pattern"
done

if [ "$missing" -ne 0 ]; then
    exit 1
fi

echo 'test matrix ok'
