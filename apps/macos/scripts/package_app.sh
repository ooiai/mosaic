#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APP_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_ROOT="$(cd "${APP_DIR}/../.." && pwd)"

APP_NAME="${APP_NAME:-Mosaic}"
APP_PRODUCT="${APP_PRODUCT:-MosaicMacApp}"
APP_EXECUTABLE_NAME="${APP_EXECUTABLE_NAME:-Mosaic}"
APP_BUNDLE_ID="${APP_BUNDLE_ID:-ai.ooiai.mosaic}"
SWIFT_CONFIGURATION="${SWIFT_CONFIGURATION:-release}"
DIST_DIR="${DIST_DIR:-${APP_DIR}/dist}"
APP_BUNDLE="${DIST_DIR}/${APP_NAME}.app"
ZIP_PATH="${DIST_DIR}/${APP_NAME}-macOS.zip"

APP_VERSION="${APP_VERSION:-$(
  sed -n 's/version = "\(.*\)"/\1/p' "${REPO_ROOT}/cli/Cargo.toml" | head -n1
)}"
APP_BUILD="${APP_BUILD:-${GITHUB_RUN_NUMBER:-1}}"

CLI_BINARY="${CLI_BINARY:-${REPO_ROOT}/cli/target/release/mosaic}"
MACOS_SIGN_IDENTITY="${MACOS_SIGN_IDENTITY:-}"
MACOS_NOTARIZE="${MACOS_NOTARIZE:-0}"
MACOS_NOTARY_PROFILE="${MACOS_NOTARY_PROFILE:-}"

if [[ -z "${APP_VERSION}" ]]; then
  APP_VERSION="$(sed -n 's/  "version": "\(.*\)",/\1/p' "${REPO_ROOT}/package.json" | head -n1)"
fi

if [[ -z "${APP_VERSION}" ]]; then
  APP_VERSION="0.0.0"
fi

if [[ "${SKIP_CLI_BUILD:-0}" != "1" ]]; then
  echo "==> Building mosaic CLI"
  (
    cd "${REPO_ROOT}/cli"
    cargo build --release -p mosaic-cli
  )
fi

if [[ "${SKIP_SWIFT_BUILD:-0}" != "1" ]]; then
  echo "==> Building macOS app binary"
  (
    cd "${APP_DIR}"
    swift build -c "${SWIFT_CONFIGURATION}" --product "${APP_PRODUCT}"
  )
fi

SWIFT_BIN_DIR="$(
  cd "${APP_DIR}"
  swift build -c "${SWIFT_CONFIGURATION}" --product "${APP_PRODUCT}" --show-bin-path
)"
APP_BINARY="${SWIFT_BIN_DIR}/${APP_PRODUCT}"

if [[ ! -x "${APP_BINARY}" ]]; then
  echo "error: expected app binary at ${APP_BINARY}" >&2
  exit 1
fi

if [[ ! -x "${CLI_BINARY}" ]]; then
  echo "error: expected CLI binary at ${CLI_BINARY}" >&2
  exit 1
fi

echo "==> Packaging ${APP_NAME}.app"
mkdir -p "${DIST_DIR}"
rm -rf "${APP_BUNDLE}"
mkdir -p \
  "${APP_BUNDLE}/Contents/MacOS" \
  "${APP_BUNDLE}/Contents/Resources/bin"

sed \
  -e "s/__APP_VERSION__/${APP_VERSION}/g" \
  -e "s/__APP_BUILD__/${APP_BUILD}/g" \
  "${APP_DIR}/Packaging/Info.plist.template" \
  > "${APP_BUNDLE}/Contents/Info.plist"

plutil -replace CFBundleIdentifier -string "${APP_BUNDLE_ID}" "${APP_BUNDLE}/Contents/Info.plist"
plutil -lint "${APP_BUNDLE}/Contents/Info.plist" >/dev/null

cp "${APP_BINARY}" "${APP_BUNDLE}/Contents/MacOS/${APP_EXECUTABLE_NAME}"
cp "${CLI_BINARY}" "${APP_BUNDLE}/Contents/Resources/bin/mosaic"
chmod +x \
  "${APP_BUNDLE}/Contents/MacOS/${APP_EXECUTABLE_NAME}" \
  "${APP_BUNDLE}/Contents/Resources/bin/mosaic"
printf 'APPL????' > "${APP_BUNDLE}/Contents/PkgInfo"

if [[ -f "${APP_DIR}/Packaging/Mosaic.icns" ]]; then
  cp "${APP_DIR}/Packaging/Mosaic.icns" "${APP_BUNDLE}/Contents/Resources/Mosaic.icns"
  plutil -replace CFBundleIconFile -string "Mosaic.icns" "${APP_BUNDLE}/Contents/Info.plist"
fi

if [[ -n "${MACOS_SIGN_IDENTITY}" ]]; then
  echo "==> Codesigning app bundle"
  codesign --force --timestamp --options runtime --sign "${MACOS_SIGN_IDENTITY}" \
    "${APP_BUNDLE}/Contents/Resources/bin/mosaic"
  codesign --force --timestamp --options runtime --sign "${MACOS_SIGN_IDENTITY}" \
    "${APP_BUNDLE}/Contents/MacOS/${APP_EXECUTABLE_NAME}"
  codesign --force --timestamp --options runtime --sign "${MACOS_SIGN_IDENTITY}" \
    "${APP_BUNDLE}"
else
  echo "==> Skipping codesign (MACOS_SIGN_IDENTITY not set)"
fi

echo "==> Creating ZIP artifact"
rm -f "${ZIP_PATH}"
ditto -c -k --sequesterRsrc --keepParent "${APP_BUNDLE}" "${ZIP_PATH}"

if [[ "${MACOS_NOTARIZE}" == "1" ]]; then
  if [[ -z "${MACOS_SIGN_IDENTITY}" || -z "${MACOS_NOTARY_PROFILE}" ]]; then
    echo "error: notarization requires MACOS_SIGN_IDENTITY and MACOS_NOTARY_PROFILE" >&2
    exit 1
  fi

  echo "==> Submitting ZIP for notarization"
  xcrun notarytool submit "${ZIP_PATH}" --keychain-profile "${MACOS_NOTARY_PROFILE}" --wait
  xcrun stapler staple "${APP_BUNDLE}"

  echo "==> Rebuilding ZIP artifact after stapling"
  rm -f "${ZIP_PATH}"
  ditto -c -k --sequesterRsrc --keepParent "${APP_BUNDLE}" "${ZIP_PATH}"
fi

echo "==> Packaged app bundle"
echo "App: ${APP_BUNDLE}"
echo "ZIP: ${ZIP_PATH}"
