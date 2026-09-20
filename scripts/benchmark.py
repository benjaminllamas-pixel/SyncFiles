#!/usr/bin/env python3
import base64
import csv
import hashlib
import json
import os
import socket
import subprocess
import sys
import tempfile
import threading
import time
import uuid
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SERVER_BINARY = Path(os.environ.get("SF_BENCH_SERVER_BIN", ROOT / "syncfiles-server/target/release/syncfiles-server"))
OUTPUT_DIR = Path(os.environ.get("SF_BENCH_OUTPUT", Path(tempfile.gettempdir()) / f"syncfiles-benchmark-{os.getpid()}"))
SERVER_URL = os.environ.get("SF_BENCH_SERVER_URL", "").rstrip("/")
EMAIL = os.environ.get("SF_BENCH_EMAIL", "bench@syncfiles.local")
PASSWORD = os.environ.get("SF_BENCH_PASSWORD", "benchpass")
DEVICE_ID = os.environ.get("SF_BENCH_DEVICE_ID", "bench-device")
SIZES = [int(value) for value in os.environ.get("SF_BENCH_SIZES", "1024,65536,1048576,5242880,10485760").split(",") if value]
ROUNDS = max(1, int(os.environ.get("SF_BENCH_ROUNDS", "3")))
CONCURRENCY = max(1, int(os.environ.get("SF_BENCH_CONCURRENCY", "4")))
REQUEST_TIMEOUT = max(1, int(os.environ.get("SF_BENCH_TIMEOUT_SECONDS", "120")))
LATENCY_SECONDS = max(0.0, float(os.environ.get("SF_BENCH_LATENCY_MS", "0")) / 1000.0)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def path_hash(path):
    return hashlib.sha256(path.encode("utf-8")).hexdigest()


def percentile(values, fraction):
    ordered = sorted(values)
    if not ordered:
        return None
    return ordered[max(0, min(len(ordered) - 1, int((len(ordered) - 1) * fraction)))]


def http_json(method, url, payload=None, token=None, timeout=REQUEST_TIMEOUT):
    data = None
    headers = {}
    if payload is not None:
        data = json.dumps(payload).encode("utf-8")
        headers["Content-Type"] = "application/json"
    if token:
        headers["Authorization"] = f"Bearer {token}"
    request = urllib.request.Request(url, data=data, headers=headers, method=method)
    started = time.perf_counter()
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            body = response.read().decode("utf-8")
            return response.status, json.loads(body), (time.perf_counter() - started) * 1000.0
    except urllib.error.HTTPError as error:
        body = error.read().decode("utf-8", errors="replace")
        raise RuntimeError(f"HTTP {error.code}: {body}") from error


def request_json(url, payload, token=None, timeout=REQUEST_TIMEOUT):
    return http_json("POST", url, payload, token, timeout)


def get_json(url, token=None, timeout=REQUEST_TIMEOUT):
    return http_json("GET", url, None, token, timeout)


def free_port():
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


def wait_ready(url, process, log_path, timeout=45):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if process.poll() is not None:
            log = log_path.read_text(errors="replace") if log_path.exists() else ""
            raise RuntimeError(f"server exited early\n{log}")
        try:
            status, _, _ = get_json(f"{url}/health/ready", timeout=2)
            if status == 200:
                return
        except Exception:
            pass
        time.sleep(0.25)
    raise RuntimeError("server did not become ready")


def start_server():
    global SERVER_URL
    if SERVER_URL:
        return None, None
    if not SERVER_BINARY.exists():
        subprocess.run(["cargo", "build", "--release", "--manifest-path", str(ROOT / "syncfiles-server/Cargo.toml")], check=True)
    work = Path(tempfile.mkdtemp(prefix="syncfiles-bench-server-"))
    db_path = work / "syncfiles.db"
    storage_path = work / "storage"
    log_path = work / "server.log"
    port = free_port()
    SERVER_URL = f"http://127.0.0.1:{port}"
    env = os.environ.copy()
    env.update({
        "SF_BIND_ADDRESS": f"127.0.0.1:{port}",
        "SF_DATABASE_URL": f"sqlite:{db_path}",
        "SF_STORAGE_ROOT": str(storage_path),
        "SF_SERVER_URL": SERVER_URL,
        "SF_MIGRATIONS_DIR": str(ROOT / "syncfiles-server/migrations"),
        "SF_STATIC_DIR": str(ROOT / "syncfiles-server/static"),
        "SF_USER_0_EMAIL": EMAIL,
        "SF_USER_0_PASSWORD": PASSWORD,
        "SF_USER_0_ID": "bench-user",
        "SF_RATE_LIMIT_RPS": "100000",
        "SF_RATE_LIMIT_BURST": "200000",
        "SF_HEALTH_PROBE_WRITE": "0",
        "RUST_LOG": "warn",
        "NO_PROXY": "127.0.0.1,localhost",
    })
    log_file = log_path.open("wb")
    process = subprocess.Popen([str(SERVER_BINARY)], stdout=log_file, stderr=subprocess.STDOUT, env=env)
    wait_ready(SERVER_URL, process, log_path)
    return process, (work, log_file)


def login():
    status, body, _ = request_json(f"{SERVER_URL}/api/v1/auth/login", {
        "email": EMAIL,
        "password": PASSWORD,
        "device_id": DEVICE_ID,
    })
    if status != 200 or not body.get("session_id"):
        raise RuntimeError(f"login failed: {body}")
    return body["session_id"]


def upload(token, size, label):
    content = os.urandom(size)
    expected_hash = sha256(content)
    relative = f"bench/{label}.bin"
    payload = {
        "session_id": token,
        "device_id": DEVICE_ID,
        "file_id": str(uuid.uuid4()),
        "relative_path": relative,
        "path_hash": path_hash(relative),
        "checksum": expected_hash,
        "size_bytes": size,
        "modified_at": int(time.time() * 1000),
        "idempotency_key": str(uuid.uuid4()),
        "content": base64.b64encode(content).decode("ascii"),
    }
    started = time.perf_counter()
    status, body, elapsed = request_json(f"{SERVER_URL}/api/v1/sync/upload", payload, token)
    duration_ms = (time.perf_counter() - started) * 1000.0
    if status != 200 or not body.get("accepted"):
        raise RuntimeError(f"upload failed: {body}")
    file_id = body.get("data", {}).get("file_id") if isinstance(body.get("data"), dict) else None
    if not file_id:
        listing_status, listing, _ = get_json(f"{SERVER_URL}/api/v1/files/list", token)
        if listing_status != 200:
            raise RuntimeError(f"upload response has no file_id: {body}")
        for entry in listing.get("files", []):
            if entry.get("relative_path") == relative:
                file_id = entry["file_id"]
                break
    if not file_id:
        raise RuntimeError(f"file_id not found for {relative}")
    return file_id, expected_hash, duration_ms, elapsed


def download(token, file_id, expected_hash, expected_size):
    payload = {
        "session_id": token,
        "device_id": DEVICE_ID,
        "file_id": file_id,
        "path_hash": "",
        "idempotency_key": str(uuid.uuid4()),
    }
    started = time.perf_counter()
    status, body, elapsed = request_json(f"{SERVER_URL}/api/v1/sync/download", payload, token)
    duration_ms = (time.perf_counter() - started) * 1000.0
    if status != 200 or not body.get("content"):
        raise RuntimeError(f"download failed: {body}")
    content = base64.b64decode(body["content"])
    if sha256(content) != expected_hash or len(content) != expected_size:
        raise RuntimeError("download checksum mismatch")
    return duration_ms, elapsed


def diff(token, since):
    payload = {
        "since": since,
        "device_id": DEVICE_ID,
        "session_id": token,
        "request_id": str(uuid.uuid4()),
    }
    _, _, elapsed = request_json(f"{SERVER_URL}/api/v1/sync/diff", payload, token)
    return elapsed


def concurrent_uploads(token, size):
    def one(index):
        _, _, duration_ms, elapsed = upload(token, size, f"concurrent-{uuid.uuid4().hex}")
        return duration_ms, elapsed

    started = time.perf_counter()
    with ThreadPoolExecutor(max_workers=CONCURRENCY) as executor:
        results = list(executor.map(one, range(CONCURRENCY)))
    total_ms = (time.perf_counter() - started) * 1000.0
    return results, total_ms


def main():
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    rows = []
    process = None
    server_state = None
    try:
        process, server_state = start_server()
        token = login()
        for size in SIZES:
            for round_number in range(1, ROUNDS + 1):
                label = f"size-{size}-round-{round_number}"
                file_id, expected_hash, upload_ms, upload_request_ms = upload(token, size, label)
                rows.append({"scenario": "single", "size_bytes": size, "operation": "upload", "round": round_number, "duration_ms": f"{upload_ms:.3f}", "request_ms": f"{upload_request_ms:.3f}", "throughput_mb_s": f"{size / upload_ms / 1024.0 if upload_ms else 0:.6f}", "status": "ok", "concurrency": 1})
                download_ms, download_request_ms = download(token, file_id, expected_hash, size)
                rows.append({"scenario": "single", "size_bytes": size, "operation": "download", "round": round_number, "duration_ms": f"{download_ms:.3f}", "request_ms": f"{download_request_ms:.3f}", "throughput_mb_s": f"{size / download_ms / 1024.0 if download_ms else 0:.6f}", "status": "ok", "concurrency": 1})
                diff_ms = diff(token, 0)
                rows.append({"scenario": "single", "size_bytes": size, "operation": "diff", "round": round_number, "duration_ms": f"{diff_ms:.3f}", "request_ms": f"{diff_ms:.3f}", "throughput_mb_s": "0", "status": "ok", "concurrency": 1})
        concurrent_size = int(os.environ.get("SF_BENCH_CONCURRENCY_SIZE", str(SIZES[0])))
        results, total_ms = concurrent_uploads(token, concurrent_size)
        for index, (duration_ms, request_ms) in enumerate(results, 1):
            rows.append({"scenario": "concurrent", "size_bytes": concurrent_size, "operation": "upload", "round": index, "duration_ms": f"{duration_ms:.3f}", "request_ms": f"{request_ms:.3f}", "throughput_mb_s": f"{concurrent_size / duration_ms / 1024.0 if duration_ms else 0:.6f}", "status": "ok", "concurrency": CONCURRENCY})
        rows.append({"scenario": "concurrent", "size_bytes": concurrent_size, "operation": "batch", "round": 1, "duration_ms": f"{total_ms:.3f}", "request_ms": f"{total_ms:.3f}", "throughput_mb_s": f"{concurrent_size * CONCURRENCY / total_ms / 1024.0 if total_ms else 0:.6f}", "status": "ok", "concurrency": CONCURRENCY})
    finally:
        if process is not None:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
        if server_state is not None:
            work, log_file = server_state
            log_file.close()
            import shutil
            shutil.rmtree(work, ignore_errors=True)

    csv_path = OUTPUT_DIR / "results.csv"
    with csv_path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=["scenario", "size_bytes", "operation", "round", "duration_ms", "request_ms", "throughput_mb_s", "status", "concurrency"])
        writer.writeheader()
        writer.writerows(rows)

    summary = {}
    groups = {}
    for row in rows:
        key = (row["scenario"], int(row["size_bytes"]), row["operation"])
        groups.setdefault(key, []).append(float(row["duration_ms"]))
    for (scenario, size, operation), values in groups.items():
        summary[f"{scenario}:{size}:{operation}"] = {
            "samples": len(values),
            "p50_ms": percentile(values, 0.50),
            "p95_ms": percentile(values, 0.95),
            "p99_ms": percentile(values, 0.99),
            "min_ms": min(values),
            "max_ms": max(values),
        }

    slo_values = [float(row["duration_ms"]) for row in rows if row["operation"] == "upload" and int(row["size_bytes"]) < 10_485_760]
    slo = {"target_p95_ms": 30000, "actual_p95_ms": percentile(slo_values, 0.95), "passed": bool(slo_values) and percentile(slo_values, 0.95) < 30000}
    json_path = OUTPUT_DIR / "summary.json"
    json_path.write_text(json.dumps({"server_url": SERVER_URL, "sizes": SIZES, "rounds": ROUNDS, "concurrency": CONCURRENCY, "slo": slo, "groups": summary}, indent=2) + "\n")
    print(f"Benchmark completado: {csv_path}")
    print(f"Resumen: {json_path}")
    print(json.dumps(slo, sort_keys=True))
    if not slo["passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
