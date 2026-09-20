#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIST="${1:-$ROOT/dist}"
PLATFORM="${2:-linux-x86_64}"

shopt -s nullglob
checksum_files=("$DIST"/*.sha256)
if [[ ${#checksum_files[@]} -eq 0 ]]; then
    printf 'No se encontraron archivos .sha256 en %s\n' "$DIST" >&2
    exit 1
fi

for checksum_file in "${checksum_files[@]}"; do
    artifact="${checksum_file%.sha256}"
    expected="$(tr -s ' ' < "$checksum_file" | cut -d' ' -f1)"
    actual="$(shasum -a 256 "$artifact" 2>/dev/null | cut -d' ' -f1 || true)"
    if [[ -z "$actual" ]] && command -v sha256sum >/dev/null 2>&1; then
        actual="$(sha256sum "$artifact" | cut -d' ' -f1)"
    fi
    if [[ "$expected" != "$actual" ]]; then
        printf 'Checksum fallido: %s\n' "$artifact" >&2
        exit 1
    fi
done

tar_archives=("$DIST"/*.tar.gz)
for archive in "${tar_archives[@]}"; do
    tar -tzf "$archive" >/dev/null
done
zip_archives=("$DIST"/*.zip)
for archive in "${zip_archives[@]}"; do
    python3 -m zipfile -t "$archive" >/dev/null
done

if [[ "$PLATFORM" == macos-* ]]; then
    for archive in "$DIST"/*.dmg; do
        if command -v codesign >/dev/null 2>&1; then
            mount_dir="$(mktemp -d "${TMPDIR:-/tmp}/syncfiles-verify-dmg.XXXXXX")"
            hdiutil attach "$archive" -mountpoint "$mount_dir" -nobrowse -quiet
            for component in server client cli; do
                codesign --verify --strict --deep "$mount_dir/syncfiles-$component"
            done
            hdiutil detach "$mount_dir" -quiet
            rm -rf "$mount_dir"
        fi
        if command -v xcrun >/dev/null 2>&1 && [[ -n "${SF_NOTARY_DRY_RUN:-}" ]]; then
            xcrun stapler validate "$archive"
        fi
    done
fi

if [[ "$PLATFORM" == windows-* ]] && command -v signtool >/dev/null 2>&1; then
    zip_archives=("$DIST"/*.zip)
    for archive in "${zip_archives[@]}"; do
        workdir="$(mktemp -d "${TMPDIR:-/tmp}/syncfiles-verify-zip.XXXXXX")"
        python3 -m zipfile -e "$archive" "$workdir" >/dev/null
        for executable in "$workdir"/*/syncfiles-*.exe; do
            [[ -e "$executable" ]] || continue
            signtool verify -pa "$executable"
        done
        rm -rf "$workdir"
    done
    for installer in "$DIST"/*.exe; do
        [[ -e "$installer" ]] || continue
        signtool verify -pa "$installer"
    done
fi

if [[ "$PLATFORM" == linux-* ]] && command -v gpg >/dev/null 2>&1; then
    for signature in "$DIST"/*.asc; do
        gpg --batch --verify "$signature" "${signature%.asc}"
    done
fi

printf 'Release verificada en %s\n' "$DIST"
