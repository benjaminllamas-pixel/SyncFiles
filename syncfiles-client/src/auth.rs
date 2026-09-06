use anyhow::Result;
use chrono::Utc;
use tracing::{info, warn};
use std::sync::Arc;

use crate::network::SyncClient;
use crate::network::LoginResponse;
use crate::metadata::MetadataStore;
use crate::metadata::SessionRecord;
use crate::metadata::Session;

pub struct AuthService {
    client: Arc<SyncClient>,
    store: Arc<MetadataStore>,
}

impl AuthService {
    pub fn new(client: Arc<SyncClient>, store: Arc<MetadataStore>) -> Self {
        Self { client, store }
    }

    pub async fn ensure_session(&self) -> Result<()> {
        if let Some(session) = self.store.get_active_session()? {
            if !session.is_expired() {
                info!("Sesión activa encontrada: {}", session.session_id);
                return Ok(());
            }
            warn!("Sesión expirada, renovando...");
        }
        self.login().await
    }

    pub async fn login(&self) -> Result<()> {
        let resp = self.client.login(&self.store.config.email, &self.store.config.password, &self.store.config.device_id).await?;
        let session = Session {
            session_id: resp.session_id.clone(),
            user_id: resp.user_id,
            device_id: resp.device_id,
            expires_at: resp.expires_at,
            token_hash: resp.session_id,
        };
        self.store.save_session(&session)?;
        info!("Login exitoso: session_id={}", session.session_id);
        Ok(())
    }

    pub async fn refresh_session(&self) -> Result<()> {
        self.store.revoke_session()?;
        self.login().await
    }
}

impl SessionRecord {
    pub fn is_expired(&self) -> bool {
        let now = Utc::now().timestamp_millis();
        now >= self.expires_at
    }
}