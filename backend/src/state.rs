use std::sync::Arc;

use sqlx::MySqlPool;
use tokio::sync::RwLock;

use crate::{
    config::Config,
    crypto::CryptoService,
    lldap::LldapClient,
    mailer::Mailer,
    metrics::MetricsStore,
    proxy::pingora::PingoraClient,
};

/// Shared application state, cloned cheaply via Arc.
#[derive(Clone)]
pub struct AppState {
    pub db: MySqlPool,
    pub config: Arc<Config>,
    pub crypto: Arc<CryptoService>,
    /// Pingora reverse-proxy client — lazy, reloaded via /admin/proxy-managers.
    pub pingora: Arc<RwLock<Option<PingoraClient>>>,
    /// JWT signing secret loaded from platform_config at startup.
    pub jwt_secret: Arc<RwLock<String>>,
    /// LLDAP admin HTTP client.
    pub lldap: Arc<LldapClient>,
    /// SMTP mailer (or dev-mode logger).
    pub mailer: Arc<Mailer>,
    /// TSDB metrics store. NullStore by default; replaced when admin configures a backend.
    pub metrics: Arc<dyn MetricsStore>,
}

impl AppState {
    pub fn new(
        db: MySqlPool,
        config: Config,
        crypto: CryptoService,
        jwt_secret: String,
        lldap: LldapClient,
        mailer: Mailer,
        metrics: Box<dyn MetricsStore>,
    ) -> Self {
        Self {
            db,
            config: Arc::new(config),
            crypto: Arc::new(crypto),
            pingora: Arc::new(RwLock::new(None)),
            jwt_secret: Arc::new(RwLock::new(jwt_secret)),
            lldap: Arc::new(lldap),
            mailer: Arc::new(mailer),
            metrics: Arc::from(metrics),
        }
    }
}
