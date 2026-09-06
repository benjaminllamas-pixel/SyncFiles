use anyhow::Result;
use crate::models::*;
use crate::db;
use crate::state::AppState;

pub async fn login(state: &AppState, req: &LoginRequest) -> Result<LoginResponse> {
    let user = db::get_user_by_email(&state.pool, &req.email).await?
        .ok_or_else(|| anyhow::anyhow!("Usuario no encontrado"))?;

    let valid = bcrypt::verify(&req.password, &user.password_hash)?;
    if !valid {
        return Err(anyhow::anyhow!("Contraseña incorrecta"));
    }

    let session_id = uuid_str();
    let token_hash = session_id.clone();
    let issued_at = now_ms();
    let expires_at = issued_at + 24 * 60 * 60 * 1000;

    let session = Session {
        session_id: session_id.clone(),
        user_id: user.user_id.clone(),
        device_id: req.device_id.clone(),
        token_hash,
        issued_at,
        expires_at,
        revoked_at: None,
        status: "active".to_string(),
    };

    sqlx::query(
        "INSERT OR IGNORE INTO devices (device_id, user_id, platform, device_name, last_seen_at, created_at) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(&req.device_id)
    .bind(&user.user_id)
    .bind("unknown")
    .bind(&req.device_id)
    .bind(issued_at)
    .bind(issued_at)
    .execute(&state.pool)
    .await?;

    db::create_session(&state.pool, &session).await?;

    db::audit_event(&state.pool, &AuditEntry {
        audit_id: uuid_str(),
        user_id: Some(user.user_id.clone()),
        device_id: Some(req.device_id.clone()),
        event_name: "auth.login_success".to_string(),
        event_type: Some("auth".to_string()),
        payload_json: None,
        created_at: now_ms(),
    }).await?;

    Ok(LoginResponse {
        session_id,
        expires_at,
        device_id: req.device_id.clone(),
        user_id: user.user_id,
    })
}

pub async fn validate_session(state: &AppState, token_hash: &str) -> Result<Session> {
    db::get_active_session(&state.pool, token_hash).await?
        .ok_or_else(|| anyhow::anyhow!("Sesión inválida o expirada"))
}

pub async fn logout(state: &AppState, session_id: &str) -> Result<()> {
    db::revoke_session(&state.pool, session_id).await?;
    Ok(())
}