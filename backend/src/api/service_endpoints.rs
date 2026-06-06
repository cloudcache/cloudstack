//! Admin CRUD + tenant-facing list for the per-kind third-party service
//! endpoints (MQ, SMTP, Redis) that templates can declare as dependencies.
//!
//! The three kinds share the same shape: name + connection details +
//! optional encrypted password + is_active. Per-kind tables (rather than a
//! generic JSON-attribute table) keep types explicit; this module factors
//! out the boilerplate.
//!
//! Routes (registered in api/mod.rs):
//!   GET/POST          /admin/mq-endpoints
//!   GET/PUT/DELETE    /admin/mq-endpoints/:id
//!   GET               /mq-endpoints                  (tenant: id+name only)
//! …same for /smtp-endpoints and /redis-endpoints.

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Extension, Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::middleware::AuthUser,
    error::{AppError, AppResult},
    state::AppState,
};

fn admin_only(auth: &AuthUser) -> AppResult<()> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("admin only".into()));
    }
    Ok(())
}

// ── MQ endpoints ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct MqUpsert {
    pub name: String,
    pub host: String,
    pub port: Option<u16>,
    pub vhost: Option<String>,
    pub username: String,
    pub password: Option<String>,
    pub tls_enabled: Option<bool>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}

pub async fn mq_list(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    admin_only(&auth)?;
    #[derive(sqlx::FromRow)]
    struct Row {
        id: String, name: String, host: String, port: u16, vhost: String,
        username: String, tls_enabled: i8, description: Option<String>, is_active: i8,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, name, host, port, vhost, username, tls_enabled, description, is_active \
         FROM mq_endpoints ORDER BY name",
    ).fetch_all(&state.db).await?;
    Ok(Json(rows.iter().map(|r| serde_json::json!({
        "id": r.id, "name": r.name, "host": r.host, "port": r.port,
        "vhost": r.vhost, "username": r.username, "tls_enabled": r.tls_enabled != 0,
        "description": r.description, "is_active": r.is_active != 0,
    })).collect::<Vec<_>>()))
}

/// Tenant-facing: minimal id+name list of active endpoints.
pub async fn mq_list_user(
    State(state): State<AppState>,
    Extension(_auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, name FROM mq_endpoints WHERE is_active = 1 ORDER BY name",
    ).fetch_all(&state.db).await?;
    Ok(Json(rows.into_iter().map(|(id, name)| serde_json::json!({"id": id, "name": name})).collect::<Vec<_>>()))
}

pub async fn mq_create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<MqUpsert>,
) -> AppResult<impl IntoResponse> {
    admin_only(&auth)?;
    let pwd_enc = body.password.as_deref().map(|p| state.crypto.encrypt(p)).transpose()?;
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO mq_endpoints \
            (id, name, host, port, vhost, username, password, tls_enabled, description, is_active) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id).bind(&body.name).bind(&body.host)
    .bind(body.port.unwrap_or(5672))
    .bind(body.vhost.as_deref().unwrap_or("/"))
    .bind(&body.username)
    .bind(&pwd_enc.unwrap_or_default())
    .bind(body.tls_enabled.unwrap_or(false) as i8)
    .bind(&body.description)
    .bind(body.is_active.unwrap_or(true) as i8)
    .execute(&state.db).await
    .map_err(unique_violation_to_conflict("mq endpoint", &body.name))?;
    Ok((axum::http::StatusCode::CREATED, Json(serde_json::json!({"id": id}))))
}

pub async fn mq_update(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<MqUpsert>,
) -> AppResult<impl IntoResponse> {
    admin_only(&auth)?;
    // Only re-encrypt password if non-empty supplied (allow leaving as-is)
    let pwd_enc = match body.password.as_deref() {
        Some("") | None => None,
        Some(p) => Some(state.crypto.encrypt(p)?),
    };
    let updated = sqlx::query(if pwd_enc.is_some() {
        "UPDATE mq_endpoints SET name=?, host=?, port=?, vhost=?, username=?, \
         password=?, tls_enabled=?, description=?, is_active=? WHERE id=?"
    } else {
        "UPDATE mq_endpoints SET name=?, host=?, port=?, vhost=?, username=?, \
         tls_enabled=?, description=?, is_active=? WHERE id=?"
    });
    let q = updated
        .bind(&body.name).bind(&body.host)
        .bind(body.port.unwrap_or(5672))
        .bind(body.vhost.as_deref().unwrap_or("/"))
        .bind(&body.username);
    let q = if let Some(p) = pwd_enc { q.bind(p) } else { q };
    q.bind(body.tls_enabled.unwrap_or(false) as i8)
        .bind(&body.description)
        .bind(body.is_active.unwrap_or(true) as i8)
        .bind(&id)
        .execute(&state.db).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn mq_delete(
    State(state): State<AppState>, Extension(auth): Extension<AuthUser>, Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    admin_only(&auth)?;
    sqlx::query("DELETE FROM mq_endpoints WHERE id = ?").bind(&id).execute(&state.db).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── SMTP endpoints ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SmtpUpsert {
    pub name: String,
    pub host: String,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from_address: Option<String>,
    pub tls_enabled: Option<bool>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}

pub async fn smtp_list(
    State(state): State<AppState>, Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    admin_only(&auth)?;
    #[derive(sqlx::FromRow)]
    struct Row {
        id: String, name: String, host: String, port: u16,
        username: Option<String>, from_address: Option<String>,
        tls_enabled: i8, description: Option<String>, is_active: i8,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, name, host, port, username, from_address, tls_enabled, description, is_active \
         FROM smtp_endpoints ORDER BY name",
    ).fetch_all(&state.db).await?;
    Ok(Json(rows.iter().map(|r| serde_json::json!({
        "id": r.id, "name": r.name, "host": r.host, "port": r.port,
        "username": r.username, "from_address": r.from_address,
        "tls_enabled": r.tls_enabled != 0, "description": r.description,
        "is_active": r.is_active != 0,
    })).collect::<Vec<_>>()))
}

pub async fn smtp_list_user(
    State(state): State<AppState>, Extension(_auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, name FROM smtp_endpoints WHERE is_active = 1 ORDER BY name",
    ).fetch_all(&state.db).await?;
    Ok(Json(rows.into_iter().map(|(id, name)| serde_json::json!({"id": id, "name": name})).collect::<Vec<_>>()))
}

pub async fn smtp_create(
    State(state): State<AppState>, Extension(auth): Extension<AuthUser>, Json(body): Json<SmtpUpsert>,
) -> AppResult<impl IntoResponse> {
    admin_only(&auth)?;
    let pwd_enc = body.password.as_deref().filter(|s| !s.is_empty()).map(|p| state.crypto.encrypt(p)).transpose()?;
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO smtp_endpoints \
            (id, name, host, port, username, password, from_address, tls_enabled, description, is_active) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id).bind(&body.name).bind(&body.host)
    .bind(body.port.unwrap_or(587))
    .bind(&body.username).bind(&pwd_enc).bind(&body.from_address)
    .bind(body.tls_enabled.unwrap_or(true) as i8)
    .bind(&body.description)
    .bind(body.is_active.unwrap_or(true) as i8)
    .execute(&state.db).await
    .map_err(unique_violation_to_conflict("smtp endpoint", &body.name))?;
    Ok((axum::http::StatusCode::CREATED, Json(serde_json::json!({"id": id}))))
}

pub async fn smtp_update(
    State(state): State<AppState>, Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>, Json(body): Json<SmtpUpsert>,
) -> AppResult<impl IntoResponse> {
    admin_only(&auth)?;
    let pwd_enc = match body.password.as_deref() {
        Some("") | None => None,
        Some(p) => Some(state.crypto.encrypt(p)?),
    };
    let sql = if pwd_enc.is_some() {
        "UPDATE smtp_endpoints SET name=?, host=?, port=?, username=?, password=?, \
         from_address=?, tls_enabled=?, description=?, is_active=? WHERE id=?"
    } else {
        "UPDATE smtp_endpoints SET name=?, host=?, port=?, username=?, \
         from_address=?, tls_enabled=?, description=?, is_active=? WHERE id=?"
    };
    let q = sqlx::query(sql)
        .bind(&body.name).bind(&body.host)
        .bind(body.port.unwrap_or(587))
        .bind(&body.username);
    let q = if let Some(p) = pwd_enc { q.bind(p) } else { q };
    q.bind(&body.from_address)
        .bind(body.tls_enabled.unwrap_or(true) as i8)
        .bind(&body.description)
        .bind(body.is_active.unwrap_or(true) as i8)
        .bind(&id)
        .execute(&state.db).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn smtp_delete(
    State(state): State<AppState>, Extension(auth): Extension<AuthUser>, Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    admin_only(&auth)?;
    sqlx::query("DELETE FROM smtp_endpoints WHERE id = ?").bind(&id).execute(&state.db).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── Redis endpoints ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RedisUpsert {
    pub name: String,
    pub host: String,
    pub port: Option<u16>,
    pub password: Option<String>,
    pub db_index: Option<i16>,
    pub tls_enabled: Option<bool>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}

pub async fn redis_list(
    State(state): State<AppState>, Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    admin_only(&auth)?;
    #[derive(sqlx::FromRow)]
    struct Row {
        id: String, name: String, host: String, port: u16, db_index: i16,
        tls_enabled: i8, description: Option<String>, is_active: i8,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, name, host, port, db_index, tls_enabled, description, is_active \
         FROM redis_endpoints ORDER BY name",
    ).fetch_all(&state.db).await?;
    Ok(Json(rows.iter().map(|r| serde_json::json!({
        "id": r.id, "name": r.name, "host": r.host, "port": r.port,
        "db_index": r.db_index, "tls_enabled": r.tls_enabled != 0,
        "description": r.description, "is_active": r.is_active != 0,
    })).collect::<Vec<_>>()))
}

pub async fn redis_list_user(
    State(state): State<AppState>, Extension(_auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, name FROM redis_endpoints WHERE is_active = 1 ORDER BY name",
    ).fetch_all(&state.db).await?;
    Ok(Json(rows.into_iter().map(|(id, name)| serde_json::json!({"id": id, "name": name})).collect::<Vec<_>>()))
}

pub async fn redis_create(
    State(state): State<AppState>, Extension(auth): Extension<AuthUser>, Json(body): Json<RedisUpsert>,
) -> AppResult<impl IntoResponse> {
    admin_only(&auth)?;
    let pwd_enc = body.password.as_deref().filter(|s| !s.is_empty()).map(|p| state.crypto.encrypt(p)).transpose()?;
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO redis_endpoints \
            (id, name, host, port, password, db_index, tls_enabled, description, is_active) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id).bind(&body.name).bind(&body.host)
    .bind(body.port.unwrap_or(6379))
    .bind(&pwd_enc)
    .bind(body.db_index.unwrap_or(0))
    .bind(body.tls_enabled.unwrap_or(false) as i8)
    .bind(&body.description)
    .bind(body.is_active.unwrap_or(true) as i8)
    .execute(&state.db).await
    .map_err(unique_violation_to_conflict("redis endpoint", &body.name))?;
    Ok((axum::http::StatusCode::CREATED, Json(serde_json::json!({"id": id}))))
}

pub async fn redis_update(
    State(state): State<AppState>, Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>, Json(body): Json<RedisUpsert>,
) -> AppResult<impl IntoResponse> {
    admin_only(&auth)?;
    let pwd_enc = match body.password.as_deref() {
        Some("") | None => None,
        Some(p) => Some(state.crypto.encrypt(p)?),
    };
    let sql = if pwd_enc.is_some() {
        "UPDATE redis_endpoints SET name=?, host=?, port=?, password=?, db_index=?, \
         tls_enabled=?, description=?, is_active=? WHERE id=?"
    } else {
        "UPDATE redis_endpoints SET name=?, host=?, port=?, db_index=?, \
         tls_enabled=?, description=?, is_active=? WHERE id=?"
    };
    let q = sqlx::query(sql)
        .bind(&body.name).bind(&body.host)
        .bind(body.port.unwrap_or(6379));
    let q = if let Some(p) = pwd_enc { q.bind(p) } else { q };
    q.bind(body.db_index.unwrap_or(0))
        .bind(body.tls_enabled.unwrap_or(false) as i8)
        .bind(&body.description)
        .bind(body.is_active.unwrap_or(true) as i8)
        .bind(&id)
        .execute(&state.db).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn redis_delete(
    State(state): State<AppState>, Extension(auth): Extension<AuthUser>, Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    admin_only(&auth)?;
    sqlx::query("DELETE FROM redis_endpoints WHERE id = ?").bind(&id).execute(&state.db).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn unique_violation_to_conflict<'a>(
    label: &'static str, name: &'a str,
) -> impl Fn(sqlx::Error) -> AppError + 'a {
    move |e| match e {
        sqlx::Error::Database(ref de) if de.is_unique_violation() => {
            AppError::Conflict(format!("{label} '{name}' already exists"))
        }
        other => AppError::Database(other),
    }
}
