#!/usr/bin/env bash
set -euo pipefail

PYTHON="${PYTHON:-python3}"
if ! command -v "$PYTHON" >/dev/null 2>&1; then
    PYTHON=python
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PLATFORM="${1:-linux-x86_64}"
OUT_DIR="${2:-$ROOT/dist}"
VERSION="${SF_VERSION:-$(git -C "$ROOT" describe --tags --always --dirty 2>/dev/null || printf 'dev')}"
VERSION="${VERSION#v}"
VERSION="${VERSION//\//_}"

mkdir -p "$OUT_DIR"
WORKDIR="$(mktemp -d "${TMPDIR:-/tmp}/syncfiles-package.XXXXXX")"
trap 'rm -rf "$WORKDIR"' EXIT

checksum256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | tr -s ' ' | cut -d' ' -f1
    elif command -v shasum >/dev/null 2>&1; then
        shasum -a 256 "$1" | tr -s ' ' | cut -d' ' -f1
    else
        "$PYTHON" - "$1" <<'PY'
import hashlib
import sys
with open(sys.argv[1], "rb") as source:
    print(hashlib.sha256(source.read()).hexdigest())
PY
    fi
}

write_checksum() {
    local artifact="$1"
    printf '%s  %s\n' "$(checksum256 "$artifact")" "$(basename "$artifact")" > "$artifact.sha256"
}

build_binary() {
    local component="$1"
    cargo build --release --manifest-path "$ROOT/syncfiles-$component/Cargo.toml"
}

stage_component() {
    local component="$1"
    local stage="$WORKDIR/syncfiles-$component"
    local binary="$ROOT/syncfiles-$component/target/release/syncfiles-$component"
    if [[ "$PLATFORM" == windows-* ]]; then
        binary="$binary.exe"
    fi
    mkdir -p "$stage"
    cp "$binary" "$stage/"
    cp "$ROOT/README.md" "$stage/README.md"
    cp "$ROOT/docs/48-documentacion-usuario.md" "$stage/DOCUMENTACION.md"
    if [[ "$component" == "cli" && -f "$ROOT/docs/55-cli-rapido.md" ]]; then
        cp "$ROOT/docs/55-cli-rapido.md" "$stage/CLI.md"
    fi
    if [[ -f "$ROOT/packaging/syncfiles-server.service" && "$component" == "server" ]]; then
        cp "$ROOT/packaging/syncfiles-server.service" "$stage/syncfiles-server.service"
    fi
    if [[ "$component" == "server" ]]; then
        cp -R "$ROOT/syncfiles-server/migrations" "$stage/migrations"
        cp -R "$ROOT/syncfiles-server/static" "$stage/static"
        mkdir -p "$stage/docs"
        cp "$ROOT/syncfiles-server/docs/openapi.md" "$stage/docs/openapi.md"
        cp "$ROOT/scripts/backup.sh" "$stage/backup.sh"
        chmod 0755 "$stage/backup.sh"
    fi
}

package_component() {
    local component="$1"
    local stage="$WORKDIR/syncfiles-$component"
    local artifact
    stage_component "$component"
    case "$PLATFORM" in
        windows-*)
            artifact="$OUT_DIR/syncfiles-$component-$VERSION-$PLATFORM.zip"
            "$PYTHON" -m zipfile -c "$artifact" -C "$WORKDIR" "syncfiles-$component"
            ;;
        macos-*)
            artifact="$OUT_DIR/syncfiles-$component-$VERSION-$PLATFORM.dmg"
            hdiutil create -srcfolder "$stage" -volname "SyncFiles-$component-$VERSION" -format UDBZ "$artifact"
            ;;
        *)
            artifact="$OUT_DIR/syncfiles-$component-$VERSION-$PLATFORM.tar.gz"
            tar -czf "$artifact" -C "$WORKDIR" "syncfiles-$component"
            ;;
    esac
    write_checksum "$artifact"
}

package_combined() {
    local combined="$WORKDIR/syncfiles-$VERSION-$PLATFORM"
    local artifact
    mkdir -p "$combined"
    for component in server client cli; do
        local binary="$ROOT/syncfiles-$component/target/release/syncfiles-$component"
        if [[ "$PLATFORM" == windows-* ]]; then
            binary="$binary.exe"
        fi
        cp "$binary" "$combined/"
    done
    cp "$ROOT/README.md" "$combined/README.md"
    cp "$ROOT/docs/48-documentacion-usuario.md" "$combined/DOCUMENTACION.md"
    cp "$ROOT/docs/55-cli-rapido.md" "$combined/CLI.md"
    cp "$ROOT/scripts/backup.sh" "$combined/backup.sh"
    chmod 0755 "$combined/backup.sh"
    if [[ -f "$ROOT/packaging/syncfiles-server.service" ]]; then
        cp "$ROOT/packaging/syncfiles-server.service" "$combined/syncfiles-server.service"
    fi
    cp -R "$ROOT/syncfiles-server/migrations" "$combined/migrations"
    cp -R "$ROOT/syncfiles-server/static" "$combined/static"
    mkdir -p "$combined/docs"
    cp "$ROOT/syncfiles-server/docs/openapi.md" "$combined/docs/openapi.md"

    case "$PLATFORM" in
        windows-*)
            artifact="$OUT_DIR/syncfiles-$VERSION-$PLATFORM.zip"
            "$PYTHON" -m zipfile -c "$artifact" -C "$WORKDIR" "syncfiles-$VERSION-$PLATFORM"
            ;;
        macos-*)
            artifact="$OUT_DIR/syncfiles-$VERSION-$PLATFORM.dmg"
            hdiutil create -srcfolder "$combined" -volname "SyncFiles-$VERSION" -format UDBZ "$artifact"
            ;;
        *)
            artifact="$OUT_DIR/syncfiles-$VERSION-$PLATFORM.tar.gz"
            tar -czf "$artifact" -C "$WORKDIR" "syncfiles-$VERSION-$PLATFORM"
            ;;
    esac
    write_checksum "$artifact"
}

package_linux_optional() {
    local combined="$WORKDIR/syncfiles-$VERSION-$PLATFORM"
    if command -v dpkg-deb >/dev/null 2>&1; then
        local deb_stage="$WORKDIR/deb"
        mkdir -p "$deb_stage/DEBIAN" "$deb_stage/opt/syncfiles/bin" "$deb_stage/lib/systemd/system"
        cp "$combined"/syncfiles-server "$deb_stage/opt/syncfiles/bin/"
        cp "$combined"/syncfiles-client "$deb_stage/opt/syncfiles/bin/"
        cp "$combined"/syncfiles-cli "$deb_stage/opt/syncfiles/bin/"
        cp "$combined"/README.md "$combined"/DOCUMENTACION.md "$deb_stage/opt/syncfiles/"
        cp -R "$combined/migrations" "$combined/static" "$deb_stage/opt/syncfiles/"
        mkdir -p "$deb_stage/opt/syncfiles/docs"
        cp "$combined/docs/openapi.md" "$deb_stage/opt/syncfiles/docs/openapi.md"
        cp "$ROOT/packaging/syncfiles-server.service" "$deb_stage/lib/systemd/system/"
        cp "$ROOT/scripts/backup.sh" "$deb_stage/opt/syncfiles/backup.sh"
        chmod 0755 "$deb_stage/opt/syncfiles/backup.sh"
        printf 'Package: syncfiles\nVersion: %s\nSection: utils\nPriority: optional\nArchitecture: amd64\nMaintainer: SyncFiles\nDescription: SyncFiles V1 synchronization server and clients\n' "$VERSION" > "$deb_stage/DEBIAN/control"
        local deb="$OUT_DIR/syncfiles-$VERSION-$PLATFORM.deb"
        dpkg-deb --build "$deb_stage" "$deb"
        write_checksum "$deb"
    fi
}

package_windows_installer() {
    local combined="$WORKDIR/syncfiles-$VERSION-$PLATFORM"
    local compiler="${INNO_SETUP_COMPILER:-}"
    if [[ -z "$compiler" ]] && command -v ISCC.exe >/dev/null 2>&1; then
        compiler="$(command -v ISCC.exe)"
    fi
    if [[ -z "$compiler" ]] && command -v powershell.exe >/dev/null 2>&1; then
        powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$ROOT/scripts/package-windows-installer.ps1" "$OUT_DIR" "$VERSION" "$combined"
    elif [[ -n "$compiler" ]]; then
        "$PYTHON" - "$compiler" "$OUT_DIR" "$VERSION" "$combined" <<'PY'
import pathlib
import subprocess
import sys

compiler, out_dir, version, combined = sys.argv[1:]
out = pathlib.Path(out_dir)
stage = pathlib.Path(combined).as_posix().replace('\\', '/')
iss = pathlib.Path(sys.argv[1]).parent / f"syncfiles-{version}-windows-x86_64-installer.iss"
iss.write_text(f'''[Setup]
AppId={{{version.replace(".", "-")}}
AppName=SyncFiles
AppVersion={version}
DefaultDirName={{autopf}}\\SyncFiles
DefaultGroupName=SyncFiles
OutputDir={out.as_posix().replace(chr(92), "/")}
OutputBaseFilename=syncfiles-{version}-windows-x86_64-setup
Compression=lzma
SolidCompression=yes
PrivilegesRequired=standard
ArchitecturesInstallIn64BitMode=x64

[Files]
Source: "{stage}\\*"; DestDir: "{{app}}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{{group}}\\SyncFiles Server"; Filename: "{{app}}\\syncfiles-server.exe"
Name: "{{group}}\\SyncFiles Client"; Filename: "{{app}}\\syncfiles-client.exe"
Name: "{{group}}\\SyncFiles CLI"; Filename: "{{app}}\\syncfiles-cli.exe"
''', encoding='utf-8')
subprocess.run([compiler, str(iss)], check=True)
PY
    else
        printf 'Advertencia: Inno Setup no está disponible; se omitió el instalador Windows.\n' >&2
    fi
    for installer in "$OUT_DIR"/syncfiles-*-windows-x86_64-setup.exe; do
        [[ -e "$installer" ]] || continue
        write_checksum "$installer"
    done
}

for component in server client cli; do
    build_binary "$component"
done

if [[ -x "$ROOT/scripts/prepare-release-signing.sh" ]]; then
    "$ROOT/scripts/prepare-release-signing.sh" "$PLATFORM"
fi

for component in server client cli; do
    package_component "$component"
done
package_combined

case "$PLATFORM" in
    linux-*) package_linux_optional "$PLATFORM" ;;
    windows-*) package_windows_installer ;;
esac

android_apk="$ROOT/syncfiles-android/app/build/outputs/apk/release/app-release.apk"
if [[ ! -f "$android_apk" ]]; then
    android_apk="$ROOT/syncfiles-android/app/build/outputs/apk/release/app-release-unsigned.apk"
fi
if [[ ! -f "$android_apk" ]]; then
    printf 'No se encontró la APK de release; ejecute assembleRelease antes de empaquetar.\n' >&2
    exit 1
fi
signed_apk="$OUT_DIR/syncfiles-android-$VERSION.apk"
cp "$android_apk" "$signed_apk"
write_checksum "$signed_apk"

printf 'Paquetes generados en %s\n' "$OUT_DIR"
