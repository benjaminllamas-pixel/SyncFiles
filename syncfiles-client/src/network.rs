#[allow(dead_code)]
use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::info;
use uuid::Uuid;

use syncfiles_models::{ApiResponse, ApiError, LoginResponse, DiffResponse, ChangeEntry, UploadRequest, DeleteRequest, DownloadRequest, ResolveConflictRequest, DownloadResponse};

#[derive(Debug, Clone)]
pub struct SyncClient {
    http: Client,
    base_url: String,
    device_id: String,
}

impl SyncClient {
    pub fn new(base_url: &str, device_id: &str) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            device_id: device_id.to_string(),
            http,
        })
    }

    fn next_idempotency_key(&self) -> String {
        Uuid::new_v4().to_string()
    }

    fn request_id(&self) -> String {
        Uuid::new_v4().to_string()
    }

    async fn request<T: for<'de> Deserialize<'de>>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<serde_json::Value>,
        auth_token: Option<&str>,
    ) -> Result<T> {
        let path = path.trim_start_matches('/');
        let url = format!("{}/{}", self.base_url, path);
        let mut req = self.http.request(method.clone(), &url);

        if let Some(b) = body {
            req = req.json(&b);
        }
        if let Some(token) = auth_token {
            req = req.bearer_auth(token);
        }

        let resp: Response = req.send().await
            .with_context(|| format!("Failed to {} {}", method, url))?;

        let status = resp.status();
        let text = resp.text().await?;

        if !status.is_success() {
            let api_err: ApiError = serde_json::from_str(&text).unwrap_or(ApiError {
                code: status.as_str().to_string(),
                message: text.clone(),
                retryable: matches!(status, StatusCode::REQUEST_TIMEOUT | StatusCode::TOO_MANY_REQUESTS | StatusCode::INTERNAL_SERVER_ERROR | StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT),
                request_id: self.request_id(),
            });
            return Err(anyhow!("API error {}: {} - retryable: {}", api_err.code, api_err.message, api_err.retryable));
        }

        let result: T = serde_json::from_str(&text)
            .with_context(|| format!("Failed to parse response from {}", path))?;
        info!("{} {} -> OK", method, path);
        Ok(result)
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn http_get_bearer(&self, url: &str, token: &str) -> Result<String> {
        let resp = self.http
            .get(url)
            .bearer_auth(token)
            .send()
            .await
            .with_context(|| format!("Failed to GET {}", url))?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(anyhow!("GET {} error {}: {}", url, status, text));
        }
        Ok(text)
    }

    pub async fn login(&self, email: &str, password: &str, device_id: &str) -> Result<LoginResponse> {
        let payload = serde_json::json!({
            "email": email,
            "password": password,
            "device_id": device_id
        });
        self.request(reqwest::Method::POST, "/api/v1/auth/login", Some(payload), None).await
    }

    pub async fn get_diff(&self, session_id: &str, since: i64) -> Result<DiffResponse> {
        let payload = serde_json::json!({
            "since": since,
            "device_id": self.device_id,
            "session_id": session_id,
            "request_id": self.request_id(),
        });
        self.request(reqwest::Method::POST, "/api/v1/sync/diff", Some(payload), Some(session_id)).await
    }

    pub async fn upload(&self, req: &UploadRequest) -> Result<ApiResponse<()>> {
        let payload = serde_json::to_value(req)?;
        let url = format!("{}/api/v1/sync/upload", self.base_url);
        let resp = self.http
            .post(&url)
            .json(&payload)
            .bearer_auth(&req.session_id)
            .send()
            .await
            .with_context(|| format!("Failed to POST {}", url))?;

        let status = resp.status();
        let text = resp.text().await?;

        if status == StatusCode::CONFLICT {
            let api_err: ApiError = serde_json::from_str(&text).unwrap_or(ApiError {
                code: "CONFLICT".to_string(),
                message: text.clone(),
                retryable: false,
                request_id: self.request_id(),
            });
            return Err(anyhow!("CONFLICT: {} - {}", api_err.code, api_err.message));
        }

        if !status.is_success() {
            let api_err: ApiError = serde_json::from_str(&text).unwrap_or(ApiError {
                code: status.as_str().to_string(),
                message: text.clone(),
                retryable: matches!(status, StatusCode::REQUEST_TIMEOUT | StatusCode::TOO_MANY_REQUESTS | StatusCode::INTERNAL_SERVER_ERROR | StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT),
                request_id: self.request_id(),
            });
            return Err(anyhow!("API error {}: {} - retryable: {}", api_err.code, api_err.message, api_err.retryable));
        }

        let result: ApiResponse<()> = serde_json::from_str(&text)
            .with_context(|| format!("Failed to parse response from /api/v1/sync/upload"))?;
        info!("POST /api/v1/sync/upload -> OK");
        Ok(result)
    }

    pub async fn download(&self, session_id: &str, file_id: &str, path_hash: &str) -> Result<DownloadResponse> {
        let payload = serde_json::json!({
            "session_id": session_id,
            "device_id": self.device_id,
            "file_id": file_id,
            "path_hash": path_hash,
            "idempotency_key": self.next_idempotency_key(),
        });
        self.request(reqwest::Method::POST, "/api/v1/sync/download", Some(payload), Some(session_id)).await
    }

    pub async fn delete(&self, session_id: &str, file_id: &str, path_hash: &str) -> Result<ApiResponse<()>> {
        let payload = DeleteRequest {
            session_id: session_id.to_string(),
            device_id: self.device_id.clone(),
            file_id: file_id.to_string(),
            path_hash: path_hash.to_string(),
            idempotency_key: self.next_idempotency_key(),
        };
        let v = serde_json::to_value(&payload)?;
        self.request(reqwest::Method::POST, "/api/v1/sync/delete", Some(v), Some(session_id)).await
    }

    pub async fn resolve_conflict(&self, session_id: &str, req: &ResolveConflictRequest) -> Result<ApiResponse<()>> {
        let payload = serde_json::to_value(req)?;
        self.request(reqwest::Method::POST, "/api/v1/conflicts/resolve", Some(payload), Some(session_id)).await
    }

    pub async fn rename(&self, session_id: &str, file_id: &str, old_path: &str, new_path: &str) -> Result<ApiResponse<()>> {
        let payload = serde_json::json!({
            "session_id": session_id,
            "device_id": self.device_id,
            "file_id": file_id,
            "old_path": old_path,
            "new_path": new_path,
            "idempotency_key": self.next_idempotency_key(),
        });
        self.request(reqwest::Method::POST, "/api/v1/sync/rename", Some(payload), Some(session_id)).await
    }

    pub async fn move_file(&self, session_id: &str, file_id: &str, old_path: &str, new_path: &str) -> Result<ApiResponse<()>> {
        let payload = serde_json::json!({
            "session_id": session_id,
            "device_id": self.device_id,
            "file_id": file_id,
            "old_path": old_path,
            "new_path": new_path,
            "idempotency_key": self.next_idempotency_key(),
        });
        self.request(reqwest::Method::POST, "/api/v1/sync/move", Some(payload), Some(session_id)).await
    }

    pub async fn copy_file(&self, session_id: &str, file_id: &str, source_path: &str, destination_path: &str) -> Result<ApiResponse<()>> {
        let payload = serde_json::json!({
            "session_id": session_id,
            "device_id": self.device_id,
            "file_id": file_id,
            "source_path": source_path,
            "destination_path": destination_path,
            "idempotency_key": self.next_idempotency_key(),
        });
        self.request(reqwest::Method::POST, "/api/v1/sync/copy", Some(payload), Some(session_id)).await
    }

    pub async fn logout(&self, session_id: &str) -> Result<ApiResponse<()>> {
        let payload = serde_json::json!({
            "session_id": session_id,
            "device_id": self.device_id,
        });
        self.request(reqwest::Method::POST, "/api/v1/auth/logout", Some(payload), Some(session_id)).await
    }

    pub async fn session_status(&self, session_id: &str) -> Result<SessionStatusResponse> {
        let url = format!("{}/api/v1/session/status", self.base_url);
        let resp = self.http
            .get(&url)
            .bearer_auth(session_id)
            .send()
            .await
            .with_context(|| format!("Failed to GET {}", url))?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(anyhow!("Session status error: {}", text));
        }
        let result: SessionStatusResponse = serde_json::from_str(&text)
            .with_context(|| "Failed to parse session status")?;
        Ok(result)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStatusResponse {
    pub session_id: String,
    pub user_id: String,
    pub device_id: String,
    pub expires_at: i64,
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    /// Mock server HTTP/1.1 mínimo: responde según el path con un closure.
    /// Cada request entrante se registra para aserciones.
    fn spawn_mock_server(
        respond: impl Fn(&str, &str, &str) -> (u16, String) + Send + Sync + 'static,
    ) -> (String, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let requests_clone = requests.clone();
        let respond = std::sync::Arc::new(respond);
        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let requests = requests_clone.clone();
                let respond = respond.clone();
                thread::spawn(move || {
                    let mut buf = [0u8; 65536];
                    let mut read = 0;
                    // Leer headers + body (Content-Length)
                    loop {
                        let n = stream.read(&mut buf[read..]).unwrap_or(0);
                        if n == 0 { break; }
                        read += n;
                        let text = String::from_utf8_lossy(&buf[..read]).to_string();
                        if let Some(pos) = text.find("\r\n\r\n") {
                            for line in text[..pos].lines() {
                                if line.to_lowercase().starts_with("content-length:") {
                                    let len: usize =
                                        line.split(':').nth(1).unwrap_or("0").trim().parse().unwrap_or(0);
                                    if read - pos - 4 >= len { break; }
                                }
                            }
                            let has_len = text[..pos].lines().any(|l| l.to_lowercase().starts_with("content-length:"));
                            if !has_len || text.len() > pos + 4 {
                                break;
                            }
                        }
                    }
                    let full = String::from_utf8_lossy(&buf[..read]).to_string();
                    let (head, body) = full.split_once("\r\n\r\n").unwrap_or((&full, ""));
                    let path = head.lines().next().unwrap_or("").split_whitespace().nth(1).unwrap_or("");
                    let auth = head
                        .lines()
                        .find(|l| l.to_lowercase().starts_with("authorization:"))
                        .unwrap_or("")
                        .to_string();
                    requests.lock().unwrap().push(format!("{} {}", head.lines().next().unwrap_or(""), body));
                    let (status, resp_body) = respond(path, body, &auth);
                    let status_line = match status {
                        200 => "200 OK",
                        400 => "400 Bad Request",
                        401 => "401 Unauthorized",
                        404 => "404 Not Found",
                        409 => "409 Conflict",
                        429 => "429 Too Many Requests",
                        500 => "500 Internal Server Error",
                        503 => "503 Service Unavailable",
                        _ => "200 OK",
                    };
                    let resp = format!(
                        "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        status_line,
                        resp_body.len(),
                        resp_body
                    );
                    let _ = stream.write_all(resp.as_bytes());
                    let _ = stream.flush();
                });
            }
        });
        (format!("http://{}", addr), requests)
    }

    fn upload_request(session_id: &str, content_b64: &str) -> UploadRequest {
        UploadRequest {
            session_id: session_id.into(),
            device_id: "device-test".into(),
            file_id: "file-1".into(),
            relative_path: "docs/a.txt".into(),
            path_hash: syncfiles_models::compute_path_hash("docs/a.txt"),
            checksum: syncfiles_models::compute_checksum(b"contenido"),
            size_bytes: 9,
            modified_at: 1_700_000_000_000,
            idempotency_key: "idem-1".into(),
            content: content_b64.to_string(),
        }
    }

    #[tokio::test]
    async fn login_success_returns_session() {
        let (url, requests) = spawn_mock_server(|path, body, _auth| {
            assert!(path.contains("/auth/login"));
            let payload: serde_json::Value = serde_json::from_str(body).unwrap();
            assert_eq!(payload["email"], "user@test.local");
            (200, r#"{"session_id":"sess-1","user_id":"u-1","device_id":"dev-1","expires_at":9999999999999}"#.into())
        });
        let client = SyncClient::new(&url, "dev-1").unwrap();
        let resp = client.login("user@test.local", "secret", "dev-1").await.unwrap();
        assert_eq!(resp.session_id, "sess-1");
        assert_eq!(resp.user_id, "u-1");
        assert_eq!(requests.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn login_invalid_credentials_returns_error() {
        let (url, _req) = spawn_mock_server(|_p, _b, _auth| {
            (400, r#"{"code":"AUTH_FAILED","message":"credenciales inválidas","retryable":false,"request_id":"r-1"}"#.into())
        });
        let client = SyncClient::new(&url, "dev-1").unwrap();
        let err = client.login("x@y.z", "mal", "dev-1").await.unwrap_err();
        assert!(err.to_string().contains("AUTH_FAILED"));
    }

    #[tokio::test]
    async fn session_status_valid_and_invalid() {
        let (url, _req) = spawn_mock_server(|path, _b, auth| {
            if auth.contains("sess-valid") && path.contains("/session/status") {
                (200, r#"{"session_id":"sess-valid","user_id":"u-1","device_id":"dev-1","expires_at":1,"status":"active"}"#.into())
            } else {
                (401, r#"{"code":"AUTH_REQUIRED","message":"sesión expirada"}"#.into())
            }
        });
        let client = SyncClient::new(&url, "dev-1").unwrap();
        let ok = client.session_status("sess-valid").await.unwrap();
        assert_eq!(ok.status, "active");
        assert!(client.session_status("sess-bad").await.is_err());
    }

    #[tokio::test]
    async fn upload_roundtrip_text_and_binary() {
        let (url, requests) = spawn_mock_server(|path, body, _auth| {
            assert!(path.contains("/sync/upload"));
            let payload: serde_json::Value = serde_json::from_str(body).unwrap();
            // El checksum declarado debe validar contra el contenido base64
            let content = payload["content"].as_str().unwrap();
            let bytes = base64::engine::general_purpose::STANDARD.decode(content).unwrap();
            let checksum = syncfiles_models::compute_checksum(&bytes);
            assert_eq!(payload["checksum"].as_str().unwrap(), checksum);
            (200, r#"{"accepted":true,"status":"uploaded","data":null,"error":null}"#.into())
        });
        let client = SyncClient::new(&url, "dev-1").unwrap();
        // Texto y binario: el checksum que calcula el mock debe validar
        for content in [b"hola mundo".to_vec(), vec![0u8, 1, 2, 255, 254, 0, 9]] {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&content);
            let mut req = upload_request("sess-1", &b64);
            req.checksum = syncfiles_models::compute_checksum(&content);
            let resp = client.upload(&req).await.unwrap();
            assert!(resp.accepted);
        }
        assert_eq!(requests.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn upload_conflict_returns_conflict_error() {
        let (url, _req) = spawn_mock_server(|_p, _b, _auth| {
            (409, r#"{"code":"CHECKSUM_CONFLICT","message":"checksum difiere","retryable":false,"request_id":"r-2"}"#.into())
        });
        let client = SyncClient::new(&url, "dev-1").unwrap();
        let req = upload_request("sess-1", "aG9sYQ==");
        let err = client.upload(&req).await.unwrap_err();
        assert!(err.to_string().starts_with("CONFLICT:"));
    }

    #[tokio::test]
    async fn download_returns_content_and_checksum() {
        let content = b"contenido binario \x00\x01";
        let checksum = syncfiles_models::compute_checksum(content);
        let expected_checksum = checksum.clone();
        let b64 = base64::engine::general_purpose::STANDARD.encode(content);
        let (url, _req) = spawn_mock_server(move |_p, _b, _auth| {
            (200, format!(r#"{{"accepted":true,"status":"ok","checksum":"{checksum}","content":"{b64}","file_id":"file-1","server_seq":10}}"#))
        });
        let client = SyncClient::new(&url, "dev-1").unwrap();
        let resp = client.download("sess-1", "file-1", "hash").await.unwrap();
        assert_eq!(resp.checksum, expected_checksum);
        let bytes = base64::engine::general_purpose::STANDARD.decode(&resp.content).unwrap();
        assert_eq!(bytes, content);
    }

    #[tokio::test]
    async fn server_error_reports_retryable_flag() {
        // 503 → el mensaje de error debe marcar retryable: true
        let (url, _req) = spawn_mock_server(|_p, _b, _auth| {
            (503, r#"{"code":"SERVICE_UNAVAILABLE","message":"caído","retryable":true,"request_id":"r-3"}"#.into())
        });
        let client = SyncClient::new(&url, "dev-1").unwrap();
        let err = client.login("a@b.c", "pw", "dev-1").await.unwrap_err();
        assert!(err.to_string().contains("retryable: true"));
        // 400 → no retryable
        let (url2, _req2) = spawn_mock_server(|_p, _b, _auth| {
            (400, r#"{"code":"VALIDATION","message":"mal request","retryable":false,"request_id":"r-4"}"#.into())
        });
        let client2 = SyncClient::new(&url2, "dev-1").unwrap();
        let err2 = client2.login("a@b.c", "pw", "dev-1").await.unwrap_err();
        assert!(err2.to_string().contains("retryable: false"));
    }

    #[tokio::test]
    async fn delete_sends_bearer_and_idempotency() {
        let (url, requests) = spawn_mock_server(|_p, _b, _auth| {
            (200, r#"{"accepted":true,"status":"deleted","data":null,"error":null}"#.into())
        });
        let client = SyncClient::new(&url, "dev-1").unwrap();
        client.delete("sess-1", "file-1", "hash-1").await.unwrap();
        let reqs = requests.lock().unwrap();
        assert!(reqs[0].contains("/api/v1/sync/delete"));
        assert!(reqs[0].contains("idempotency_key"));
    }

    #[tokio::test]
    async fn get_diff_posts_since_cursor() {
        let (url, requests) = spawn_mock_server(|_p, body, _auth| {
            let payload: serde_json::Value = serde_json::from_str(body).unwrap();
            assert_eq!(payload["since"], 41);
            (200, r#"{"changes":[],"server_seq":41,"total":0}"#.into())
        });
        let client = SyncClient::new(&url, "dev-1").unwrap();
        let diff = client.get_diff("sess-1", 41).await.unwrap();
        assert_eq!(diff.server_seq, 41);
        assert!(diff.changes.is_empty());
        assert!(requests.lock().unwrap()[0].contains("/api/v1/sync/diff"));
    }
}