#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
missing=0

for path in \
    .env.example \
    docs/deployment.md \
    docs/operations.md \
    docs/security.md \
    docs/release.md \
    docs/testing.md \
    docs/telegram-step-by-step.md \
    docs/compatibility.md \
    docs/upgrade.md \
    examples/deployment/README.md \
    examples/deployment/production.config.yaml \
    examples/deployment/mosaic.service \
    scripts/release-smoke.sh \
    scripts/test-golden-examples.sh \
    scripts/test-real-integrations.sh \
    scripts/verify-test-matrix.sh \
    scripts/verify-delivery-artifacts.sh
 do
    if [ ! -f "$ROOT/$path" ]; then
        echo "missing artifact: $path" >&2
        missing=1
    fi
done

for path in \
    scripts/release-smoke.sh \
    scripts/test-golden-examples.sh \
    scripts/test-real-integrations.sh \
    scripts/verify-test-matrix.sh \
    scripts/verify-delivery-artifacts.sh
do
    if [ ! -x "$ROOT/$path" ]; then
        echo "artifact is not executable: $path" >&2
        missing=1
    fi
done

if [ "$missing" -ne 0 ]; then
    exit 1
fi

echo 'delivery artifacts ok'
