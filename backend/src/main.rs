#![recursion_limit = "256"]

mod api;
mod auth;
mod billing;
mod config;
mod crypto;
mod db;
mod docker;
mod error;
mod k8s;
mod lldap;
mod mailer;
mod metrics;
mod proxy;
mod quota;
mod rate_limit;
mod ssh;
mod templates;
mod state;
mod storage_guard;

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

    // ─── Seed PUBLIC app templates if app_templates table is empty ───────────
    if let Err(e) = templates::seed_if_missing(&state).await {
        tracing::warn!("template seed failed: {e}");
    }

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

    // ─── Clean up expired tokens (hourly) ───────────────────────────────────
    {
        let db = state.db.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
            loop {
                interval.tick().await;
                let _ = sqlx::query("DELETE FROM refresh_tokens WHERE expires_at < NOW()")
                    .execute(&db).await;
                let _ = sqlx::query("DELETE FROM user_sessions WHERE expires_at < NOW()")
                    .execute(&db).await;
            }
        });
    }

    // ─── Recover stale PROVISIONING nodes on startup ──────────────────────────
    // If the process crashed mid-provision, nodes remain stuck in PROVISIONING.
    // Mark any node whose last provision log is older than 15 minutes as failed.
    {
        let db = state.db.clone();
        let rows = sqlx::query_scalar::<_, String>(
            "SELECT id FROM cluster_nodes WHERE node_status = 'PROVISIONING'",
        )
        .fetch_all(&db)
        .await
        .unwrap_or_default();

        for nid in &rows {
            // Count log rows for this node's current attempt
            let log_count: i64 = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM node_provision_logs WHERE node_id = ?",
            )
            .bind(nid)
            .fetch_one(&db)
            .await
            .unwrap_or(0);

            // Case 1: No logs at all — legacy node or provision never started
            // Case 2: Has RUNNING logs older than 15 min — stalled mid-provision
            let stale = if log_count == 0 {
                true
            } else {
                sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) FROM node_provision_logs \
                     WHERE node_id = ? AND status = 'RUNNING' \
                     AND started_at < DATE_SUB(NOW(), INTERVAL 15 MINUTE)",
                )
                .bind(nid)
                .fetch_one(&db)
                .await
                .unwrap_or(0)
                > 0
            };

            if stale {
                tracing::warn!("recovering stale PROVISIONING node {nid}");
                let _ = sqlx::query(
                    "UPDATE cluster_nodes SET node_status = 'NOT_READY', \
                     provision_step = 'failed', \
                     provision_error = 'process restarted during provisioning' \
                     WHERE id = ? AND node_status = 'PROVISIONING'"
                )
                .bind(nid)
                .execute(&db)
                .await;
                // Mark any RUNNING log rows as FAILED too
                let _ = sqlx::query(
                    "UPDATE node_provision_logs SET status = 'FAILED', finished_at = NOW() \
                     WHERE node_id = ? AND status = 'RUNNING'"
                )
                .bind(nid)
                .execute(&db)
                .await;
            }
        }

        if !rows.is_empty() {
            tracing::info!("checked {} PROVISIONING node(s) on startup", rows.len());
        }
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
                    tracing::debug!("status sync (k8s): {e}");
                }
                if let Err(e) = docker::sync_docker_app_statuses(&sync_state).await {
                    tracing::debug!("status sync (docker): {e}");
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

    // ─── Background LDAP user sync ───────────────────────────────────────────
    // Keeps the local user index aligned with LDAP after LDAP has been configured.
    if cfg.ldap.sync_interval_secs > 0 && !cfg.ldap.url.trim().is_empty() {
        let ldap_state = state.clone();
        let interval_secs = cfg.ldap.sync_interval_secs;
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(interval_secs));
            loop {
                interval.tick().await;
                match auth::ldap_sync::sync_ldap_users(&ldap_state.db, &ldap_state.config.ldap).await {
                    Ok(report) => {
                        if report.conflicts.is_empty() {
                            tracing::debug!(
                                scanned = report.scanned,
                                inserted = report.inserted,
                                updated = report.updated,
                                skipped = report.skipped,
                                "LDAP user sync completed"
                            );
                        } else {
                            tracing::warn!(
                                scanned = report.scanned,
                                inserted = report.inserted,
                                updated = report.updated,
                                skipped = report.skipped,
                                conflicts = report.conflicts.len(),
                                "LDAP user sync completed with conflicts"
                            );
                        }
                    }
                    Err(e) => tracing::debug!("LDAP user sync skipped: {e}"),
                }
            }
        });
    }

    // ─── Router ──────────────────────────────────────────────────────────────
    // ─── CORS ─────────────────────────────────────────────────────────────
    // Read allowed origins from config file [server].cors_origins.
    // Falls back to permissive mode only when the list is empty (dev mode).
    let cors = {
        let origins: Vec<String> = cfg.server.cors_origins.iter()
            .map(|o| o.trim().trim_end_matches('/').to_string())
            .filter(|o| !o.is_empty())
            .collect();

        let cors_base = CorsLayer::new()
            .allow_methods([
                axum::http::Method::GET,
                axum::http::Method::POST,
                axum::http::Method::PUT,
                axum::http::Method::DELETE,
                axum::http::Method::OPTIONS,
            ])
            .allow_headers([
                axum::http::header::CONTENT_TYPE,
                axum::http::header::AUTHORIZATION,
                axum::http::header::ACCEPT,
            ]);

        if origins.is_empty() {
            tracing::warn!("server.cors_origins is empty — CORS allows all origins (dev mode)");
            cors_base.allow_origin(tower_http::cors::Any)
        } else {
            let allowed: Vec<axum::http::HeaderValue> = origins.iter()
                .filter_map(|o| o.parse().ok())
                .collect();
            tracing::info!("CORS allowed origins: {:?}", origins);
            cors_base.allow_origin(allowed)
        }
    };

    let app = api::router(state.clone())
        .layer(TraceLayer::new_for_http())
        .layer(cors)
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
