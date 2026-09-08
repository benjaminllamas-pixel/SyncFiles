use anyhow::Result;
use chrono::Utc;
use tracing::{info, warn};
use std::sync::{Arc, Mutex};

use crate::network::SyncClient;
use crate::metadata::MetadataStore;
use crate::metadata::Session;
use crate::metadata::SessionRecord;

pub struct AuthService {
    client: Arc<SyncClient>,
    store: Arc<Mutex<MetadataStore>>,
}

impl AuthService {
    pub fn new(client: Arc<SyncClient>, store: Arc<Mutex<MetadataStore>>) -> Self {
        Self { client, store }
    }

    pub async fn ensure_session(&self) -> Result<()> {
        let session = {
            let store = self.store.lock().unwrap();
            store.get_active_session()?
        };
        if let Some(session) = session {
            if !session.is_expired() {
                info!("Sesión activa encontrada: {}", session.session_id);
                return Ok(());
            }
            warn!("Sesión expirada, renovando...");
        }
        self.login().await
    }

    pub async fn login(&self) -> Result<()> {
        let (email, password, device_id) = {
            let store = self.store.lock().unwrap();
            (store.config.email.clone(), store.config.password.clone(), store.config.device_id.clone())
        };
        let resp = self.client.login(&email, &password, &device_id).await?;
        let session = Session {
            session_id: resp.session_id.clone(),
            user_id: resp.user_id,
            device_id: resp.device_id,
            expires_at: resp.expires_at,
            token_hash: resp.session_id,
        };
        {
            let store = self.store.lock().unwrap();
            store.save_session(&session)?;
        }
        info!("Login exitoso: session_id={}", session.session_id);
        Ok(())
    }

    pub async fn login_raw(&self, email: &str, password: &str, device_id: &str) -> Result<()> {
        let resp = self.client.login(email, password, device_id).await?;
        let session = Session {
            session_id: resp.session_id.clone(),
            user_id: resp.user_id,
            device_id: resp.device_id,
            expires_at: resp.expires_at,
            token_hash: resp.session_id.clone(),
        };
        {
            let store = self.store.lock().unwrap();
            store.save_session(&session)?;
        }
        info!("Login exitoso: session_id={}", session.session_id);
        Ok(())
    }

    pub async fn refresh_session(&self) -> Result<()> {
        {
            let store = self.store.lock().unwrap();
            store.revoke_session()?;
        }
        self.login().await
    }
}