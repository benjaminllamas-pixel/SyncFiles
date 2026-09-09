use anyhow::Result;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use reqwest::Client;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::Duration;
use syncfiles_models::{compute_checksum, compute_path_hash, now_ms};
use uuid::Uuid;

struct Args {
    server_url: String,
    email: String,
    password: String,
    device_id: String,
    sync_root: PathBuf,
    command: String,
    arg1: String,
    arg2: String,
}

fn parse_args() -> Args {
    let args: Vec<String> = std::env::args().collect();
    let get = |idx: usize, env: &str, default: &str| -> String {
        if args.len() > idx {
            args[idx].clone()
        } else {
            std::env::var(env).unwrap_or_else(|_| default.to_string())
        }
    };
    Args {
        server_url: get(1, "SF_SERVER_URL", "http://127.0.0.1:8080"),
        email: get(2, "SF_EMAIL", "admin@syncfiles.local"),
        password: get(3, "SF_PASSWORD", "syncfiles"),
        device_id: get(4, "SF_DEVICE_ID", "cli-test"),
        sync_root: PathBuf::from(get(5, "SF_SYNC_ROOT", "/tmp/syncfiles-cli")),
        command: get(6, "SF_CMD", "login"),
        arg1: get(7, "SF_ARG1", ""),
        arg2: get(8, "SF_ARG2", ""),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args();
    let http = Client::builder().timeout(Duration::from_secs(30)).build()?;
    let base = args.server_url.trim_end_matches('/').to_string();

    match args.command.as_str() {
        "login" => {
            let resp = do_login(&http, &base, &args.email, &args.password, &args.device_id).await?;
            println!("{}", resp);
        }
        "upload" => {
            let session = do_login(&http, &base, &args.email, &args.password, &args.device_id).await?;
            let content = std::fs::read(&args.arg1)?;
            let checksum = compute_checksum(&content);
            let relative = PathBuf::from(&args.arg1)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string();
            let path_hash = compute_path_hash(&relative);
            let payload = serde_json::json!({
                "session_id": session,
                "device_id": args.device_id,
                "file_id": Uuid::new_v4(),
                "relative_path": relative,
                "path_hash": path_hash,
                "checksum": checksum,
                "size_bytes": content.len(),
                "modified_at": now_ms(),
                "idempotency_key": Uuid::new_v4(),
                "content": BASE64.encode(&content),
            });
            let resp = http.post(format!("{}/api/v1/sync/upload", base))
                .json(&payload)
                .bearer_auth(&session)
                .send()
                .await?;
            println!("{}", resp.text().await?);
        }
        "download" => {
            let session = do_login(&http, &base, &args.email, &args.password, &args.device_id).await?;
            let payload = serde_json::json!({
                "session_id": session,
                "device_id": args.device_id,
                "file_id": args.arg1,
                "path_hash": "",
                "idempotency_key": Uuid::new_v4(),
            });
            let resp = http.post(format!("{}/api/v1/sync/download", base))
                .json(&payload)
                .bearer_auth(&session)
                .send()
                .await?;
            let v: Value = resp.json().await?;
            if let Some(content) = v.get("content").and_then(|c| c.as_str()) {
                let bytes = BASE64.decode(content)?;
                std::fs::write(&args.arg2, &bytes)?;
                println!("Saved {} bytes to {}", bytes.len(), args.arg2);
            }
            println!("{}", serde_json::to_string_pretty(&v)?);
        }
        "delete" => {
            let session = do_login(&http, &base, &args.email, &args.password, &args.device_id).await?;
            let payload = serde_json::json!({
                "session_id": session,
                "device_id": args.device_id,
                "file_id": args.arg1,
                "path_hash": "",
                "idempotency_key": Uuid::new_v4(),
            });
            let resp = http.post(format!("{}/api/v1/sync/delete", base))
                .json(&payload)
                .bearer_auth(&session)
                .send()
                .await?;
            println!("{}", resp.text().await?);
        }
        "rename" => {
            let session = do_login(&http, &base, &args.email, &args.password, &args.device_id).await?;
            let payload = serde_json::json!({
                "session_id": session,
                "device_id": args.device_id,
                "file_id": args.arg1,
                "old_path": "",
                "new_path": args.arg2,
                "idempotency_key": Uuid::new_v4(),
            });
            let resp = http.post(format!("{}/api/v1/sync/rename", base))
                .json(&payload)
                .bearer_auth(&session)
                .send()
                .await?;
            println!("{}", resp.text().await?);
        }
        "diff" => {
            let session = do_login(&http, &base, &args.email, &args.password, &args.device_id).await?;
            let since: i64 = args.arg1.parse().unwrap_or(0);
            let payload = serde_json::json!({
                "since": since,
                "device_id": args.device_id,
                "session_id": session,
                "request_id": Uuid::new_v4(),
            });
            let resp = http.post(format!("{}/api/v1/sync/diff", base))
                .json(&payload)
                .bearer_auth(&session)
                .send()
                .await?;
            println!("{}", resp.text().await?);
        }
        "session-status" => {
            let session = do_login(&http, &base, &args.email, &args.password, &args.device_id).await?;
            let resp = http.get(format!("{}/api/v1/session/status", base))
                .bearer_auth(&session)
                .send()
                .await?;
            println!("{}", resp.text().await?);
        }
        "logout" => {
            let session = do_login(&http, &base, &args.email, &args.password, &args.device_id).await?;
            let payload = serde_json::json!({
                "session_id": session,
                "device_id": args.device_id,
            });
            let resp = http.post(format!("{}/api/v1/auth/logout", base))
                .json(&payload)
                .bearer_auth(&session)
                .send()
                .await?;
            println!("{}", resp.text().await?);
        }
        other => {
            eprintln!("Unknown command: {}", other);
            std::process::exit(1);
        }
    }
    Ok(())
}

async fn do_login(http: &Client, base: &str, email: &str, password: &str, device_id: &str) -> Result<String> {
    let payload = serde_json::json!({
        "email": email,
        "password": password,
        "device_id": device_id,
    });
    let resp = http.post(format!("{}/api/v1/auth/login", base))
        .json(&payload)
        .send()
        .await?;
    if !resp.status().is_success() {
        let text = resp.text().await?;
        anyhow::bail!("Login failed: {}", text);
    }
    let v: Value = resp.json().await?;
    Ok(v["session_id"].as_str().unwrap().to_string())
}