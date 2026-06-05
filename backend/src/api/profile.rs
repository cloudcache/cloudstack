/// User self-service profile API.
///
/// All endpoints require authentication; they operate on the calling user's
/// own data only (no path-level user ID — the user is taken from the JWT).
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Extension, Json,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    auth::middleware::AuthUser,
    crypto::CryptoService,
    error::{AppError, AppResult},
    state::AppState,
};

// ─── Profile read / update ────────────────────────────────────────────────────

pub async fn get_profile(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    let user = sqlx::query!(
        r#"SELECT id, username, email, display_name, is_global_admin, created_at
           FROM users WHERE id = ?"#,
        auth.user_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("user not found".into()))?;

    let wallet = sqlx::query!(
        r#"SELECT balance, currency FROM user_wallets WHERE user_id = ?"#,
        auth.user_id
    )
    .fetch_optional(&state.db)
    .await?;

    let totp_enabled: bool = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM totp_credentials WHERE user_id = ? AND enabled = 1"#,
        auth.user_id
    )
    .fetch_one(&state.db)
    .await? > 0;

    let subscription = sqlx::query!(
        r#"SELECT s.id, s.status, s.billing_cycle, s.expires_at, s.auto_renew,
                  p.name AS plan_name, p.display_name AS plan_display_name,
                  p.quota_cpu_mcores, p.quota_mem_mb, p.quota_storage_gb,
                  p.quota_bandwidth_gb, p.quota_domain_count, p.quota_db_instance_count,
                  p.quota_project_count, p.quota_app_count, p.quota_request_million
           FROM user_subscriptions s
           JOIN subscription_plans p ON p.id = s.plan_id
           WHERE s.user_id = ? AND s.status IN ('ACTIVE','OVERDUE','PENDING')
           ORDER BY s.created_at DESC LIMIT 1"#,
        auth.user_id
    )
    .fetch_optional(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "id": user.id,
        "username": user.username,
        "email": user.email,
        "display_name": user.display_name,
        "is_global_admin": user.is_global_admin != 0,
        "subscription": subscription.map(|s| serde_json::json!({
            "id": s.id,
            "status": s.status,
            "billing_cycle": s.billing_cycle,
            "expires_at": s.expires_at,
            "auto_renew": s.auto_renew != 0,
            "plan_name": s.plan_name,
            "plan_display_name": s.plan_display_name,
            "plan_quota": {
                "cpu_mcores":        s.quota_cpu_mcores,
                "mem_mb":            s.quota_mem_mb,
                "storage_gb":        s.quota_storage_gb,
                "bandwidth_gb":      s.quota_bandwidth_gb,
                "domain_count":      s.quota_domain_count,
                "db_instance_count": s.quota_db_instance_count,
                "project_count":     s.quota_project_count,
                "app_count":         s.quota_app_count,
                "request_million":   s.quota_request_million,
            },
        })),
        "wallet": wallet.map(|w| serde_json::json!({
            "balance": w.balance,
            "currency": w.currency,
        })),
        "totp_enabled": totp_enabled,
        "created_at": user.created_at,
    })))
}

pub async fn update_profile(
    State(_state): State<AppState>,
    Extension(_auth): Extension<AuthUser>,
) -> AppResult<axum::http::StatusCode> {
    Err(AppError::Forbidden(
        "profile identity fields are admin-managed; users may only manage SSH keys".into(),
    ))
}

// ─── Change password ──────────────────────────────────────────────────────────

pub async fn change_password(
    State(_state): State<AppState>,
    Extension(_auth): Extension<AuthUser>,
) -> AppResult<axum::http::StatusCode> {
    Err(AppError::Forbidden(
        "password changes are admin-managed; users may only manage SSH keys".into(),
    ))
}

// ─── Sessions ─────────────────────────────────────────────────────────────────

pub async fn list_sessions(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    let rows = sqlx::query!(
        r#"SELECT id, ip_addr, user_agent, created_at, expires_at
           FROM user_sessions
           WHERE user_id = ? AND expires_at > NOW()
           ORDER BY created_at DESC"#,
        auth.user_id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!(rows.iter().map(|r| serde_json::json!({
        "id": r.id,
        "ip_addr": r.ip_addr,
        "user_agent": r.user_agent,
        "created_at": r.created_at,
        "expires_at": r.expires_at,
    })).collect::<Vec<_>>())))
}

pub async fn revoke_session(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(session_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let result = sqlx::query!(
        r#"DELETE FROM user_sessions WHERE id = ? AND user_id = ?"#,
        session_id,
        auth.user_id,
    )
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("session {session_id}")));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn revoke_all_sessions(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    axum_extra::TypedHeader(bearer): axum_extra::TypedHeader<
        axum_extra::headers::Authorization<axum_extra::headers::authorization::Bearer>,
    >,
) -> AppResult<impl IntoResponse> {
    // Keep the current session alive
    let current_hash = CryptoService::sha256_hex(bearer.token());
    sqlx::query!(
        r#"DELETE FROM user_sessions WHERE user_id = ? AND token_hash != ?"#,
        auth.user_id,
        current_hash,
    )
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── SSH keys ─────────────────────────────────────────────────────────────────

pub async fn list_ssh_keys(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    let rows = sqlx::query!(
        r#"SELECT id, name, fingerprint, created_at FROM user_ssh_keys
           WHERE user_id = ? ORDER BY created_at DESC"#,
        auth.user_id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!(rows.iter().map(|r| serde_json::json!({
        "id": r.id,
        "name": r.name,
        "fingerprint": r.fingerprint,
        "created_at": r.created_at,
    })).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct AddSshKeyRequest {
    pub name: String,
    pub public_key: String,
}

pub async fn add_ssh_key(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<AddSshKeyRequest>,
) -> AppResult<impl IntoResponse> {
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("key name is required".into()));
    }

    let public_key = body.public_key.trim().to_string();
    let fingerprint = compute_ssh_fingerprint(&public_key)
        .ok_or_else(|| AppError::BadRequest("invalid SSH public key format".into()))?;

    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO user_ssh_keys (id, user_id, name, public_key, fingerprint)
           VALUES (?, ?, ?, ?, ?)"#,
        id,
        auth.user_id,
        body.name.trim(),
        public_key,
        fingerprint,
    )
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref de) if de.is_unique_violation() => {
            AppError::Conflict("this SSH key is already added to your account".into())
        }
        other => AppError::Database(other),
    })?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "id": id, "fingerprint": fingerprint })),
    ))
}

pub async fn get_ssh_key(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(key_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let row = sqlx::query!(
        r#"SELECT id, name, public_key, fingerprint, created_at
           FROM user_ssh_keys WHERE id = ? AND user_id = ?"#,
        key_id,
        auth.user_id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("ssh key {key_id}")))?;

    Ok(Json(serde_json::json!({
        "id": row.id,
        "name": row.name,
        "public_key": row.public_key,
        "fingerprint": row.fingerprint,
        "created_at": row.created_at,
    })))
}

#[derive(Deserialize)]
pub struct UpdateSshKeyRequest {
    pub name: Option<String>,
    pub public_key: Option<String>,
}

pub async fn update_ssh_key(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(key_id): Path<String>,
    Json(body): Json<UpdateSshKeyRequest>,
) -> AppResult<impl IntoResponse> {
    if body.name.as_deref().is_some_and(|name| name.trim().is_empty()) {
        return Err(AppError::BadRequest("key name is required".into()));
    }

    let public_key = body.public_key.as_ref().map(|key| key.trim().to_string());
    let fingerprint = if let Some(public_key) = public_key.as_deref() {
        Some(
            compute_ssh_fingerprint(public_key)
                .ok_or_else(|| AppError::BadRequest("invalid SSH public key format".into()))?,
        )
    } else {
        None
    };

    let result = sqlx::query(
        r#"UPDATE user_ssh_keys
           SET name = COALESCE(?, name),
               public_key = COALESCE(?, public_key),
               fingerprint = COALESCE(?, fingerprint)
           WHERE id = ? AND user_id = ?"#,
    )
    .bind(body.name.as_deref().map(str::trim))
    .bind(&public_key)
    .bind(&fingerprint)
    .bind(&key_id)
    .bind(&auth.user_id)
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref de) if de.is_unique_violation() => {
            AppError::Conflict("this SSH key is already added to your account".into())
        }
        other => AppError::Database(other),
    })?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("ssh key {key_id}")));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn delete_ssh_key(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(key_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let result = sqlx::query!(
        r#"DELETE FROM user_ssh_keys WHERE id = ? AND user_id = ?"#,
        key_id,
        auth.user_id,
    )
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("ssh key {key_id}")));
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Compute SSH public key fingerprint in `SHA256:base64 (key-type)` format.
fn compute_ssh_fingerprint(public_key: &str) -> Option<String> {
    use base64::Engine;
    let parts: Vec<&str> = public_key.trim().splitn(3, ' ').collect();
    if parts.len() < 2 {
        return None;
    }
    let key_type = parts[0];
    let key_bytes = base64::engine::general_purpose::STANDARD
        .decode(parts[1])
        .ok()?;

    let hash = Sha256::digest(&key_bytes);
    let fp = base64::engine::general_purpose::STANDARD_NO_PAD.encode(hash);
    Some(format!("SHA256:{fp} ({key_type})"))
}
