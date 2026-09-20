#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PLATFORM="${1:-linux-x86_64}"

if [[ "$PLATFORM" == macos-* && -n "${SF_APPLE_CERT_BASE64:-}" ]]; then
    WORKDIR="$(mktemp -d "${TMPDIR:-/tmp}/syncfiles-signing.XXXXXX")"
    KEYCHAIN="$WORKDIR/syncfiles-signing.keychain"
    KEYCHAIN_PASSWORD="$(python3 -c 'import secrets; print(secrets.token_urlsafe(32))')"
    trap 'security delete-keychain "$KEYCHAIN" 2>/dev/null || true; rm -rf "$WORKDIR"' EXIT
    printf '%s' "$SF_APPLE_CERT_BASE64" | python3 -c 'import base64, sys; sys.stdout.buffer.write(base64.b64decode(sys.stdin.read()))' > "$WORKDIR/certificate.p12"
    security create-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN"
    security set-keychain-settings -lut 21600 "$KEYCHAIN"
    security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN"
    security import "$WORKDIR/certificate.p12" -k "$KEYCHAIN" -P "${SF_APPLE_CERT_PASSWORD:-}" -T /usr/bin/codesign
    security list-keychains -d user -s "$KEYCHAIN"
    for component in server client cli; do
        codesign --force --deep --sign "$SF_APPLE_IDENTITY" "$ROOT/syncfiles-$component/target/release/syncfiles-$component"
    done
fi

if [[ "$PLATFORM" == windows-* && -n "${SF_WINDOWS_PFX_BASE64:-}" ]]; then
    WORKDIR="$(mktemp -d "${TMPDIR:-/tmp}/syncfiles-signing.XXXXXX")"
    trap 'rm -rf "$WORKDIR"' EXIT
    printf '%s' "$SF_WINDOWS_PFX_BASE64" | python3 -c 'import base64, sys; sys.stdout.buffer.write(base64.b64decode(sys.stdin.read()))' > "$WORKDIR/certificate.pfx"
    for component in server client cli; do
        signtool sign /fd SHA256 /f "$WORKDIR/certificate.pfx" /p "${SF_WINDOWS_CERT_PASSWORD:-}" "$ROOT/syncfiles-$component/target/release/syncfiles-$component.exe"
    done
fi
