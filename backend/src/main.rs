#![recursion_limit = "256"]

mod api;
mod auth;
mod billing;
mod config;
mod crypto;
mod db;
mod error;
mod k8s;
mod lldap;
mod mailer;
mod metrics;
mod proxy;
mod quota;
mod ssh;
mod state;

use std::net::SocketAddr;

use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::{
    config::Config,
    crypto::CryptoService,
    lldap::LldapClient,
    mailer::Mailer,
    proxy::pingora::PingoraClient,
    state::AppState,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ─── Logging ─────────────────────────────────────────────────────────────
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,quickstack=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // ─── Config ───────────────────────────────────────────────────────────────
    let cfg = Config::load().map_err(|e| {
        tracing::error!("failed to load config: {e}");
        e
    })?;

    // ─── Crypto ───────────────────────────────────────────────────────────────
    // QS_ENCRYPTION_KEY env var takes priority; falls back to config [crypto].key
    let encryption_key = std::env::var("QS_ENCRYPTION_KEY")
        .unwrap_or_else(|_| cfg.crypto.key.clone());
    let crypto = CryptoService::new(&encryption_key)?;

    // ─── Database ─────────────────────────────────────────────────────────────
    tracing::info!("connecting to database...");
    let db = db::connect(&cfg.database).await?;
    tracing::info!("database connected, migrations applied");

    // ─── JWT secret ──────────────────────────────────────────────────────────
    let jwt_secret = load_jwt_secret(&db, &crypto).await?;

    // ─── LLDAP admin client ──────────────────────────────────────────────────
    let lldap = LldapClient::new(
        &cfg.ldap.http_url,
        &cfg.ldap.admin_username,
        &cfg.ldap.bind_password,
    );

    // ─── Mailer ──────────────────────────────────────────────────────────────
    let mailer = Mailer::new(&cfg.smtp)?;

    // ─── Metrics store ───────────────────────────────────────────────────────
    // NullStore until admin configures a TSDB backend via platform_config.
    // We need a temporary state with no metrics to bootstrap the load call.
    let bootstrap_state = AppState::new(
        db.clone(), cfg.clone(), crypto.clone(), jwt_secret.clone(),
        LldapClient::new(&cfg.ldap.http_url, &cfg.ldap.admin_username, &cfg.ldap.bind_password),
        Mailer::new(&cfg.smtp)?,
        Box::new(metrics::NullStore),
    );
    let metrics_store = metrics::load_store(&bootstrap_state).await;

    // ─── AppState ────────────────────────────────────────────────────────────
    let state = AppState::new(db.clone(), cfg.clone(), crypto.clone(), jwt_secret, lldap, mailer, metrics_store);

    // ─── Pingora client ──────────────────────────────────────────────────────
    if let Some(pm) = load_proxy_manager(&db, &crypto).await? {
        *state.pingora.write().await = Some(pm);
        tracing::info!("pingora-proxy-manager client initialised");
    } else {
        tracing::warn!("no active proxy_managers row found — proxy features disabled until configured");
    }

    // ─── Background quota enforcer ────────────────────────────────────────────
    {
        let enforcer_state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                if let Err(e) = quota::run_enforcer(&enforcer_state).await {
                    tracing::warn!("quota enforcer error: {e}");
                }
            }
        });
    }

    // ─── Background billing tasks (5-min interval) ───────────────────────────
    {
        let billing_state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(300));
            loop {
                interval.tick().await;
                billing::run_billing_tasks(&billing_state).await;
            }
        });
    }

    // ─── Background app status sync (30-sec interval) ────────────────────────
    // Watches K8s for DEPLOYING apps and transitions them to RUNNING / FAILED.
    {
        let sync_state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            loop {
                interval.tick().await;
                if let Err(e) = k8s::sync_app_statuses(&sync_state).await {
                    tracing::debug!("status sync: {e}");
                }
            }
        });
    }

    // ─── Background node health poll (2-min interval) ────────────────────────
    // Pings each registered node's kubelet / metrics-server and updates last_seen_at.
    {
        let node_state = state.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(120));
            loop {
                interval.tick().await;
                poll_node_health(&node_state).await;
            }
        });
    }

    // ─── Router ──────────────────────────────────────────────────────────────
    let app = api::router(state.clone())
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::PUT,
                    axum::http::Method::DELETE,
                    axum::http::Method::OPTIONS,
                ])
                .allow_headers(tower_http::cors::Any),
        )
        .with_state(state);

    // ─── Listen ──────────────────────────────────────────────────────────────
    let addr: SocketAddr = format!("{}:{}", cfg.server.host, cfg.server.port).parse()?;
    tracing::info!("QuickStack listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

/// Background task: poll K8s Node objects for each cluster and sync node_status + last_seen_at.
async fn poll_node_health(state: &AppState) {
    use k8s_openapi::api::core::v1::Node;
    use kube::Api;

    // Fetch all cluster IDs that have a kubeconfig
    let cluster_ids = match sqlx::query_scalar!(
        r#"SELECT id FROM clusters WHERE kubeconfig IS NOT NULL"#
    )
    .fetch_all(&state.db)
    .await {
        Ok(ids) => ids,
        Err(e) => {
            tracing::debug!("node health poll: failed to list clusters: {e}");
            return;
        }
    };

    for cluster_id in cluster_ids {
        let kube = match k8s::client_for_cluster(state, &cluster_id).await {
            Ok(c) => c,
            Err(e) => {
                tracing::debug!("node health poll cluster {cluster_id}: {e}");
                continue;
            }
        };

        let nodes: Api<Node> = Api::all(kube);
        let node_list = match nodes.list(&kube::api::ListParams::default()).await {
            Ok(l) => l,
            Err(e) => {
                tracing::debug!("node health poll for cluster {cluster_id}: {e}");
                continue;
            }
        };

        for node in node_list.items {
            let hostname = match node.metadata.name.as_deref() {
                Some(h) => h.to_string(),
                None => continue,
            };

            let is_ready = node.status
                .as_ref()
                .and_then(|s| s.conditions.as_ref())
                .map(|conds| conds.iter().any(|c| c.type_ == "Ready" && c.status == "True"))
                .unwrap_or(false);

            let status = if is_ready { "READY" } else { "NOT_READY" };

            let _ = sqlx::query!(
                r#"UPDATE cluster_nodes
                   SET node_status = ?, last_seen_at = NOW()
                   WHERE hostname = ?"#,
                status, hostname,
            )
            .execute(&state.db)
            .await;
        }
    }
}

async fn load_jwt_secret(
    db: &sqlx::MySqlPool,
    crypto: &CryptoService,
) -> anyhow::Result<String> {
    let row = sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = 'jwt_secret'"#
    )
    .fetch_optional(db)
    .await?;

    if let Some(encrypted) = row {
        if !encrypted.is_empty() {
            return Ok(crypto.decrypt(&encrypted)?);
        }
    }

    use rand::Rng;
    let secret: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();

    let encrypted = crypto.encrypt(&secret)?;
    sqlx::query!(
        r#"INSERT INTO platform_config (`key`, `value`, description)
           VALUES ('jwt_secret', ?, 'JWT signing secret (auto-generated)')
           ON DUPLICATE KEY UPDATE `value` = ?"#,
        encrypted,
        encrypted,
    )
    .execute(db)
    .await?;

    tracing::info!("generated new JWT secret");
    Ok(secret)
}

async fn load_proxy_manager(
    db: &sqlx::MySqlPool,
    crypto: &CryptoService,
) -> anyhow::Result<Option<PingoraClient>> {
    let row = sqlx::query!(
        r#"SELECT api_base_url, api_username, api_password
           FROM proxy_managers WHERE is_active = 1 LIMIT 1"#
    )
    .fetch_optional(db)
    .await?;

    let Some(pm) = row else {
        return Ok(None);
    };

    let password = crypto.decrypt(&pm.api_password)?;
    Ok(Some(PingoraClient::new(
        &pm.api_base_url,
        &pm.api_username,
        &password,
    )))
}
