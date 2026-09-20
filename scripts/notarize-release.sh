#!/usr/bin/env bash
set -euo pipefail

DIST="${1:-$ROOT/dist}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APPLE_ID="${SF_APPLE_ID:-}"
APPLE_PASSWORD="${SF_APPLE_APP_PASSWORD:-}"
APPLE_TEAM_ID="${SF_APPLE_TEAM_ID:-}"
PROFILE="${SF_NOTARY_PROFILE:-SyncFilesNotary}"

if [[ "${SF_NOTARY_DRY_RUN:-0}" == "1" ]]; then
    printf 'Notarization dry-run: %s\n' "$DIST"
    exit 0
fi

if [[ -z "$APPLE_ID" || -z "$APPLE_PASSWORD" || -z "$APPLE_TEAM_ID" ]]; then
    printf 'Faltan SF_APPLE_ID, SF_APPLE_APP_PASSWORD o SF_APPLE_TEAM_ID.\n' >&2
    exit 1
fi

if ! command -v xcrun >/dev/null 2>&1; then
    printf 'xcrun no está disponible en este sistema.\n' >&2
    exit 1
fi

xcrun notarytool store-credentials "$PROFILE" \
    --apple-id "$APPLE_ID" \
    --password "$APPLE_PASSWORD" \
    --team-id "$APPLE_TEAM_ID"

for archive in "$DIST"/*.dmg; do
    [[ -e "$archive" ]] || continue
    xcrun notarytool submit "$archive" --keychain-profile "$PROFILE" --wait
    xcrun stapler staple "$archive"
    xcrun stapler validate "$archive"
done
