use axum::{extract::{Path, State}, response::IntoResponse, Extension, Json};
use uuid::Uuid;
use serde::Deserialize;

use crate::{auth::middleware::AuthUser, error::{AppError, AppResult}, state::AppState};

pub async fn list(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;
    let rows = sqlx::query!(
        r#"SELECT id, hostname, target_port, is_system_domain,
                  ssl_enabled, cert_status, cert_expiry, redirect_https
           FROM app_domains WHERE app_id = ? ORDER BY hostname"#,
        app_id
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(serde_json::json!(rows.iter().map(|r| serde_json::json!({
        "id": r.id,
        "hostname": r.hostname,
        "target_port": r.target_port,
        "is_system_domain": r.is_system_domain != 0,
        "ssl_enabled": r.ssl_enabled != 0,
        "cert_status": r.cert_status,
        "cert_expiry": r.cert_expiry,
        "redirect_https": r.redirect_https != 0,
    })).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct CreateDomainRequest {
    pub hostname: String,
    pub target_port: u16,
    pub ssl_enabled: Option<bool>,
    pub use_lets_encrypt: Option<bool>,
}

pub async fn create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
    Json(body): Json<CreateDomainRequest>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;

    let ssl = body.ssl_enabled.unwrap_or(true);
    let le = body.use_lets_encrypt.unwrap_or(true);

    // Get nodeport for target_port
    let nodeport = sqlx::query_scalar!(
        r#"SELECT nodeport FROM app_ports WHERE app_id = ? AND container_port = ?"#,
        app_id,
        body.target_port,
    )
    .fetch_optional(&state.db)
    .await?
    .flatten()
    .ok_or_else(|| AppError::BadRequest("port not found or not yet deployed".into()))?;

    // Get a K3s node IP for pingora upstream
    let node_ip = sqlx::query_scalar!(
        r#"SELECT ip_address FROM cluster_nodes WHERE node_status = 'READY' LIMIT 1"#
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Internal("no ready nodes".into()))?;

    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO app_domains (id, app_id, hostname, target_port, ssl_enabled, use_lets_encrypt)
           VALUES (?, ?, ?, ?, ?, ?)"#,
        id, app_id, body.hostname, body.target_port, ssl as i8, le as i8,
    )
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref de) if de.is_unique_violation() => {
            AppError::Conflict(format!("hostname '{}' already in use", body.hostname))
        }
        other => AppError::Database(other),
    })?;

    // Register with pingora
    if let Some(pingora) = state.pingora.read().await.as_ref() {
        let req = crate::proxy::pingora::CreateHostRequest {
            domain: body.hostname.clone(),
            scheme: if ssl { "https".into() } else { "http".into() },
            upstream_ip: node_ip,
            upstream_port: nodeport,
            ssl_force: ssl,
        };
        if let Err(e) = pingora.create_host(&req).await {
            tracing::warn!("pingora create_host failed for {}: {e}", body.hostname);
        }
        if ssl && le {
            if let Err(e) = pingora.request_cert(&body.hostname).await {
                tracing::warn!("pingora cert request failed for {}: {e}", body.hostname);
            }
        }
    }

    Ok((axum::http::StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

pub async fn delete_domain(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, _app_id, domain_id)): Path<(String, String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;

    let hostname = sqlx::query_scalar!(
        r#"SELECT hostname FROM app_domains WHERE id = ?"#,
        domain_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("domain {domain_id}")))?;

    if let Some(pingora) = state.pingora.read().await.as_ref() {
        if let Err(e) = pingora.delete_host(&hostname).await {
            tracing::warn!("pingora delete_host failed for {hostname}: {e}");
        }
    }

    sqlx::query!(r#"DELETE FROM app_domains WHERE id = ?"#, domain_id)
        .execute(&state.db)
        .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}
