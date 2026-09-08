#[allow(dead_code)]
use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::info;
use uuid::Uuid;

use syncfiles_models::{ApiResponse, ApiError, LoginRequest, LoginResponse, DiffRequest, DiffResponse, ChangeEntry, UploadRequest, DeleteRequest, DownloadRequest, ResolveConflictRequest, DownloadResponse, now_ms};

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
        self.request(reqwest::Method::POST, "/api/v1/sync/upload", Some(payload), Some(&req.session_id)).await
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