#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="${1:-$ROOT/dist}"
PLATFORM="${2:-linux-x86_64}"

if [[ "$PLATFORM" == linux-* && -n "${SF_GPG_PRIVATE_KEY:-}" ]]; then
    WORKDIR="$(mktemp -d "${TMPDIR:-/tmp}/syncfiles-gpg.XXXXXX")"
    trap 'rm -rf "$WORKDIR"' EXIT
    chmod 700 "$WORKDIR"
    export GNUPGHOME="$WORKDIR"
    printf '%s' "$SF_GPG_PRIVATE_KEY" | python3 -c 'import base64, sys; sys.stdout.buffer.write(base64.b64decode(sys.stdin.read()))' > "$WORKDIR/private.key"
    gpg --batch --import "$WORKDIR/private.key"
    for archive in "$DIST"/*.tar.gz "$DIST"/*.zip; do
        [[ -e "$archive" ]] || continue
        [[ "$archive" == *.sha256 ]] && continue
        gpg --batch --yes --pinentry-mode loopback --passphrase "${SF_GPG_PASSPHRASE:-}" --armor --detach-sign "$archive"
    done
fi

verify_linux() {
    local archive
    for archive in "$DIST"/*.tar.gz "$DIST"/*.zip; do
        [[ -e "$archive" ]] || continue
        [[ "$archive" == *.sha256 ]] && continue
        if [[ -f "$archive.asc" && -n "${SF_GPG_PRIVATE_KEY:-}" ]]; then
            gpg --batch --verify "$archive.asc" "$archive"
        fi
    done
}

verify_macos() {
    local archive mount_dir
    for archive in "$DIST"/*.dmg; do
        [[ -e "$archive" ]] || continue
        mount_dir="$(mktemp -d "${TMPDIR:-/tmp}/syncfiles-dmg.XXXXXX")"
        hdiutil attach "$archive" -mountpoint "$mount_dir" -nobrowse -quiet
        for component in server client cli; do
            codesign --verify --strict --deep "$mount_dir/syncfiles-$component"
        done
        hdiutil detach "$mount_dir" -quiet
        rm -rf "$mount_dir"
    done
}

verify_windows() {
    local archive workdir
    for archive in "$DIST"/*.zip; do
        [[ -e "$archive" ]] || continue
        workdir="$(mktemp -d "${TMPDIR:-/tmp}/syncfiles-zip.XXXXXX")"
        python3 -m zipfile -e "$archive" "$workdir"
        for component in server client cli; do
            signtool verify -pa "$workdir"/*/syncfiles-"$component".exe
        done
        rm -rf "$workdir"
    done
    for installer in "$DIST"/*.exe; do
        [[ -e "$installer" ]] || continue
        signtool verify -pa "$installer"
    done
}

case "$PLATFORM" in
    linux-*) verify_linux ;;
    macos-*) verify_macos ;;
    windows-*) verify_windows ;;
esac
