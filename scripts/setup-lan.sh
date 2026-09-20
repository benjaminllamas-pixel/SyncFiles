#!/usr/bin/env bash
set -euo pipefail

PORT="${SF_LAN_PORT:-8080}"
INTERFACE=""
SERVER_IP=""
ACTION="info"
ANDROID_IP=""

usage() {
    printf 'Uso: %s [--port 8080] [--interface en0] [--server-ip IP] [--check] [--open-firewall] [--android-ip IP]\n' "$0"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --port)
            PORT="$2"
            shift 2
            ;;
        --interface)
            INTERFACE="$2"
            shift 2
            ;;
        --server-ip)
            SERVER_IP="$2"
            shift 2
            ;;
        --android-ip)
            ANDROID_IP="$2"
            shift 2
            ;;
        --check)
            ACTION="check"
            shift
            ;;
        --open-firewall)
            ACTION="firewall"
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            usage >&2
            exit 2
            ;;
    esac
done

detect_ip() {
    if [[ -n "$INTERFACE" ]]; then
        if command -v ipconfig >/dev/null 2>&1; then
            ipconfig getifaddr "$INTERFACE" 2>/dev/null || true
        elif command -v ip >/dev/null 2>&1; then
            ip -4 -o addr show dev "$INTERFACE" | python3 -c 'import sys; print(sys.stdin.read().split()[3].split("/")[0])' 2>/dev/null || true
        fi
    fi
    if [[ -z "$SERVER_IP" ]]; then
        if command -v ipconfig >/dev/null 2>&1; then
            for interface in en0 en1 en2 utun0; do
                SERVER_IP="$(ipconfig getifaddr "$interface" 2>/dev/null || true)"
                if [[ -n "$SERVER_IP" && "$SERVER_IP" != "0.0.0.0" ]]; then
                    break
                fi
            done
        fi
        if [[ -z "$SERVER_IP" ]] && command -v ip >/dev/null 2>&1; then
            SERVER_IP="$(ip route get 1.1.1.1 2>/dev/null | python3 -c 'import sys; words=sys.stdin.read().split(); print(words[words.index("src") + 1])' 2>/dev/null || true)"
        fi
        if [[ -z "$SERVER_IP" ]] && command -v hostname >/dev/null 2>&1; then
            SERVER_IP="$(hostname -I 2>/dev/null | python3 -c 'import sys; print(sys.stdin.read().split()[0])' 2>/dev/null || true)"
        fi
    fi
    if [[ -z "$SERVER_IP" ]]; then
        printf 'No se pudo detectar una IP LAN. Use --server-ip.\n' >&2
        exit 1
    fi
}

is_private_ip() {
    python3 - "$1" <<'PY'
import ipaddress
import sys
try:
    address = ipaddress.ip_address(sys.argv[1])
except ValueError:
    raise SystemExit(1)
raise SystemExit(0 if address.is_private else 1)
PY
}

print_info() {
    detect_ip
    printf 'URL del servidor: http://%s:%s\n' "$SERVER_IP" "$PORT"
    printf 'Arranque bare-metal:\n'
    printf '  SF_BIND_ADDRESS=0.0.0.0:%s SF_SERVER_URL=http://%s:%s ./syncfiles-server/target/release/syncfiles-server\n' "$PORT" "$SERVER_IP" "$PORT"
    printf 'Android debug: use la APK debug; el manifiesto debug permite cleartext solo para pruebas LAN.\n'
    printf 'Android release: use HTTPS o regenere la APK con una entrada de red específica para %s.\n' "$SERVER_IP"
    printf 'Desktop: export SF_SERVER_URL=http://%s:%s\n' "$SERVER_IP" "$PORT"
}

check_server() {
    detect_ip
    if curl -fsS --max-time 5 "http://$SERVER_IP:$PORT/health/live" >/dev/null; then
        printf 'Servidor accesible en http://%s:%s\n' "$SERVER_IP" "$PORT"
    else
        printf 'No se pudo alcanzar http://%s:%s/health/live\n' "$SERVER_IP" "$PORT" >&2
        exit 1
    fi
}

open_firewall() {
    detect_ip
    if command -v ufw >/dev/null 2>&1; then
        sudo ufw allow "$PORT/tcp"
    elif command -v firewall-cmd >/dev/null 2>&1; then
        sudo firewall-cmd --permanent --add-port="$PORT/tcp"
        sudo firewall-cmd --reload
    elif command -v pfctl >/dev/null 2>&1; then
        printf 'Añada una regla pf para %s/tcp; pfctl requiere una regla explícita y permisos de administrador.\n' "$PORT"
    elif command -v netsh >/dev/null 2>&1; then
        netsh advfirewall firewall add rule name="SyncFiles LAN" dir=in action=allow protocol=TCP localport="$PORT"
    else
        printf 'No se detectó ufw, firewall-cmd, pfctl ni netsh. Abra %s/tcp manualmente.\n' "$PORT" >&2
        exit 1
    fi
}

if [[ -n "$ANDROID_IP" ]]; then
    if ! is_private_ip "$ANDROID_IP"; then
        printf 'La IP de Android debe ser una dirección privada de la LAN.\n' >&2
        exit 1
    fi
    printf '<domain-config cleartextTrafficPermitted="true">\n'
    printf '    <domain>%s</domain>\n' "$ANDROID_IP"
    printf '</domain-config>\n'
    exit 0
fi

case "$ACTION" in
    info) print_info ;;
    check) check_server ;;
    firewall) open_firewall ;;
esac
