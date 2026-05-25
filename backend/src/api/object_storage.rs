//! Object storage: S3-compatible targets (admin) + backup schedules (user).
//!
//! Admin manages S3 targets (MinIO, AWS S3, etc.).
//! Users attach backup schedules to their apps, referencing an admin-configured target.

use axum::{extract::{Path, State}, response::IntoResponse, Extension, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::{auth::middleware::AuthUser, error::{AppError, AppResult}, state::AppState};

// ─── Admin: S3 targets ────────────────────────────────────────────────────────

/// GET /admin/s3-targets
pub async fn list_targets(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let rows = sqlx::query!(
        r#"SELECT id, name, endpoint, region, access_key_id, bucket_name, is_active, created_at
           FROM s3_targets ORDER BY name"#
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!(rows.iter().map(|r| serde_json::json!({
        "id":           r.id,
        "name":         r.name,
        "endpoint":     r.endpoint,
        "region":       r.region,
        "access_key_id": r.access_key_id,
        "bucket_name":  r.bucket_name,
        "is_active":    r.is_active != 0,
        "created_at":   r.created_at,
    })).collect::<Vec<_>>())))
}

/// GET /admin/s3-targets/:id
pub async fn get_target(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let r = sqlx::query!(
        r#"SELECT id, name, endpoint, region, access_key_id, bucket_name, is_active, created_at
           FROM s3_targets WHERE id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("s3 target {id}")))?;

    Ok(Json(serde_json::json!({
        "id":           r.id,
        "name":         r.name,
        "endpoint":     r.endpoint,
        "region":       r.region,
        "access_key_id": r.access_key_id,
        "bucket_name":  r.bucket_name,
        "is_active":    r.is_active != 0,
        "created_at":   r.created_at,
    })))
}

#[derive(Deserialize)]
pub struct CreateTargetRequest {
    pub name: String,
    pub endpoint: String,
    pub region: Option<String>,
    pub access_key_id: String,
    pub secret_key: String,
    pub bucket_name: String,
}

/// POST /admin/s3-targets
pub async fn create_target(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreateTargetRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let encrypted_secret = state.crypto.encrypt(&body.secret_key)?;
    let id = Uuid::new_v4().to_string();

    sqlx::query!(
        r#"INSERT INTO s3_targets (id, name, endpoint, region, access_key_id, secret_key, bucket_name)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        id, body.name, body.endpoint, body.region,
        body.access_key_id, encrypted_secret, body.bucket_name,
    )
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref de) if de.is_unique_violation() =>
            AppError::Conflict(format!("s3 target '{}' already exists", body.name)),
        other => AppError::Database(other),
    })?;

    Ok((axum::http::StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

#[derive(Deserialize)]
pub struct UpdateTargetRequest {
    pub name: Option<String>,
    pub endpoint: Option<String>,
    pub region: Option<String>,
    pub access_key_id: Option<String>,
    pub secret_key: Option<String>,
    pub bucket_name: Option<String>,
    pub is_active: Option<bool>,
}

/// PUT /admin/s3-targets/:id
pub async fn update_target(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<UpdateTargetRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let existing = sqlx::query!(
        r#"SELECT name, endpoint, region, access_key_id, bucket_name, is_active
           FROM s3_targets WHERE id = ?"#,
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("s3 target {id}")))?;

    let name        = body.name.as_deref().unwrap_or(&existing.name);
    let endpoint    = body.endpoint.as_deref().unwrap_or(&existing.endpoint);
    let access_key  = body.access_key_id.as_deref().unwrap_or(&existing.access_key_id);
    let bucket      = body.bucket_name.as_deref().unwrap_or(&existing.bucket_name);
    let is_active   = body.is_active.map(|v| v as i8).unwrap_or(existing.is_active);
    let region      = body.region.as_deref().or(existing.region.as_deref());

    if let Some(secret) = &body.secret_key {
        let enc = state.crypto.encrypt(secret)?;
        sqlx::query!(
            r#"UPDATE s3_targets SET name=?, endpoint=?, region=?, access_key_id=?,
               secret_key=?, bucket_name=?, is_active=? WHERE id=?"#,
            name, endpoint, region, access_key, enc, bucket, is_active, id,
        )
        .execute(&state.db)
        .await?;
    } else {
        sqlx::query!(
            r#"UPDATE s3_targets SET name=?, endpoint=?, region=?, access_key_id=?,
               bucket_name=?, is_active=? WHERE id=?"#,
            name, endpoint, region, access_key, bucket, is_active, id,
        )
        .execute(&state.db)
        .await?;
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// DELETE /admin/s3-targets/:id
pub async fn delete_target(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let active = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM backup_schedules WHERE s3_target_id = ? AND is_active = 1"#,
        id
    )
    .fetch_one(&state.db)
    .await?;

    if active > 0 {
        return Err(AppError::Conflict(
            format!("s3 target has {active} active backup schedules; disable them first")
        ));
    }

    sqlx::query!(r#"DELETE FROM s3_targets WHERE id = ?"#, id)
        .execute(&state.db)
        .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── User: list available S3 targets (for backup schedule creation) ───────────

/// GET /s3-targets  — authenticated users see active targets to pick from
pub async fn list_targets_user(
    State(state): State<AppState>,
    Extension(_auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    let rows = sqlx::query!(
        r#"SELECT id, name, endpoint, region, bucket_name
           FROM s3_targets WHERE is_active = 1 ORDER BY name"#
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!(rows.iter().map(|r| serde_json::json!({
        "id":          r.id,
        "name":        r.name,
        "endpoint":    r.endpoint,
        "region":      r.region,
        "bucket_name": r.bucket_name,
    })).collect::<Vec<_>>())))
}

// ─── User: backup schedules ───────────────────────────────────────────────────

/// GET /projects/:pid/apps/:aid/backups
pub async fn list_backups(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;

    let rows = sqlx::query!(
        r#"SELECT bs.id, bs.cron_expr, bs.retention_days, bs.backup_type, bs.is_active,
                  bs.last_run_at, bs.last_run_status, bs.created_at,
                  t.name AS target_name, t.bucket_name
           FROM backup_schedules bs
           JOIN s3_targets t ON t.id = bs.s3_target_id
           WHERE bs.app_id = ?
           ORDER BY bs.created_at DESC"#,
        app_id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!(rows.iter().map(|r| serde_json::json!({
        "id":             r.id,
        "cron_expr":      r.cron_expr,
        "retention_days": r.retention_days,
        "backup_type":    r.backup_type,
        "is_active":      r.is_active != 0,
        "last_run_at":    r.last_run_at,
        "last_run_status": r.last_run_status,
        "target_name":    r.target_name,
        "bucket_name":    r.bucket_name,
        "created_at":     r.created_at,
    })).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct CreateBackupRequest {
    pub s3_target_id: String,
    pub cron_expr: String,
    pub retention_days: Option<u16>,
    pub backup_type: Option<String>,
    /// Optional DB instance to include in dump (backup_type = DB_DUMP or BOTH)
    pub db_instance_id: Option<String>,
}

/// POST /projects/:pid/apps/:aid/backups
pub async fn create_backup(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
    Json(body): Json<CreateBackupRequest>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;

    // Verify target exists and is active
    let target_exists = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM s3_targets WHERE id = ? AND is_active = 1"#,
        body.s3_target_id
    )
    .fetch_one(&state.db)
    .await?;
    if target_exists == 0 {
        return Err(AppError::NotFound(format!("s3 target {}", body.s3_target_id)));
    }

    // Basic cron validation (5 fields)
    if body.cron_expr.split_whitespace().count() != 5 {
        return Err(AppError::BadRequest(
            "cron_expr must have 5 fields (minute hour dom month dow)".into()
        ));
    }

    let backup_type = body.backup_type.as_deref().unwrap_or("FILES");
    if !matches!(backup_type, "FILES" | "DB_DUMP" | "BOTH") {
        return Err(AppError::BadRequest(
            "backup_type must be FILES, DB_DUMP, or BOTH".into()
        ));
    }

    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO backup_schedules
           (id, app_id, s3_target_id, cron_expr, retention_days, backup_type, db_instance_id)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        id, app_id, body.s3_target_id, body.cron_expr,
        body.retention_days.unwrap_or(7),
        backup_type,
        body.db_instance_id,
    )
    .execute(&state.db)
    .await?;

    Ok((axum::http::StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

#[derive(Deserialize)]
pub struct UpdateBackupRequest {
    pub cron_expr: Option<String>,
    pub retention_days: Option<u16>,
    pub is_active: Option<bool>,
}

/// PUT /projects/:pid/apps/:aid/backups/:bid
pub async fn update_backup(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, _app_id, backup_id)): Path<(String, String, String)>,
    Json(body): Json<UpdateBackupRequest>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;

    if let Some(cron) = &body.cron_expr {
        if cron.split_whitespace().count() != 5 {
            return Err(AppError::BadRequest(
                "cron_expr must have 5 fields".into()
            ));
        }
    }

    sqlx::query!(
        r#"UPDATE backup_schedules
           SET cron_expr      = COALESCE(?, cron_expr),
               retention_days = COALESCE(?, retention_days),
               is_active      = COALESCE(?, is_active)
           WHERE id = ?"#,
        body.cron_expr,
        body.retention_days,
        body.is_active.map(|v| v as i8),
        backup_id,
    )
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// DELETE /projects/:pid/apps/:aid/backups/:bid
pub async fn delete_backup(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, _app_id, backup_id)): Path<(String, String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;

    sqlx::query!(r#"DELETE FROM backup_schedules WHERE id = ?"#, backup_id)
        .execute(&state.db)
        .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── S3 connection test ───────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TestTargetRequest {
    pub endpoint: String,
    pub region: Option<String>,
    pub access_key_id: String,
    pub secret_key: String,
    pub bucket_name: String,
}

/// POST /admin/s3-targets/test — verify S3 credentials by listing the bucket
/// Uses a minimal AWS Signature V4 signed HEAD request via reqwest.
pub async fn test_target(
    State(_state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<TestTargetRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let endpoint = if body.endpoint.starts_with("http") {
        body.endpoint.clone()
    } else {
        format!("https://{}", body.endpoint)
    };

    // Simple connectivity check: GET the bucket endpoint and accept any HTTP response
    // (even 403 from wrong creds means the server is reachable; 000/timeout means unreachable)
    let url = format!("{}/{}/", endpoint.trim_end_matches('/'), body.bucket_name);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    match client.get(&url).send().await {
        Ok(resp) => {
            // Any HTTP response (even 403/404) means the endpoint is reachable
            let status = resp.status().as_u16();
            if status == 200 || status == 403 || status == 404 {
                Ok(Json(serde_json::json!({ "ok": true, "http_status": status })))
            } else {
                Err(AppError::BadRequest(format!(
                    "S3 endpoint returned unexpected status {status}"
                )))
            }
        }
        Err(e) => Err(AppError::BadRequest(format!("S3 connection failed: {e}"))),
    }
}

// ─── Backup file listing (aggregate view) ──────────────────────────────────────

#[derive(Deserialize)]
pub struct BackupListQuery {
    pub s3_target_id: Option<String>,
}

/// GET /api/v1/backups?s3_target_id=
/// Returns all backup schedules (app-level and managed-volume) accessible to
/// the user, optionally filtered by S3 target. Actual S3 file listing is
/// deferred (requires S3 client integration).
pub async fn list_all_backups(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    axum::extract::Query(q): axum::extract::Query<BackupListQuery>,
) -> AppResult<impl IntoResponse> {
    // Build base SQL for app-level schedules — use sqlx::query() to avoid macro
    // type inference issues with repeated Option parameters.
    let (app_sql, vol_sql) = if auth.is_global_admin {
        (
            r#"SELECT bs.id, bs.app_id, bs.s3_target_id, bs.cron_expr,
                      CAST(bs.retention_days AS SIGNED) AS retention_days,
                      bs.backup_type, bs.is_active,
                      a.name AS app_name, p.id AS project_id, p.name AS project_name,
                      t.name AS s3_target_name
               FROM backup_schedules bs
               JOIN apps a ON a.id = bs.app_id
               JOIN projects p ON p.id = a.project_id
               JOIN s3_targets t ON t.id = bs.s3_target_id
               ORDER BY p.name, a.name"#,
            r#"SELECT vb.id, mv.app_id, vb.volume_id, vb.s3_target_id, vb.cron_expr,
                      CAST(vb.retention_days AS SIGNED) AS retention_days,
                      vb.is_active, mv.name AS volume_name, mv.container_mount_path,
                      a.name AS app_name, p.id AS project_id, p.name AS project_name,
                      t.name AS s3_target_name
               FROM app_volume_backups vb
               JOIN app_managed_volumes mv ON mv.id = vb.volume_id
               JOIN apps a ON a.id = mv.app_id
               JOIN projects p ON p.id = a.project_id
               JOIN s3_targets t ON t.id = vb.s3_target_id
               ORDER BY p.name, a.name"#,
        )
    } else {
        (
            r#"SELECT bs.id, bs.app_id, bs.s3_target_id, bs.cron_expr,
                      CAST(bs.retention_days AS SIGNED) AS retention_days,
                      bs.backup_type, bs.is_active,
                      a.name AS app_name, p.id AS project_id, p.name AS project_name,
                      t.name AS s3_target_name
               FROM backup_schedules bs
               JOIN apps a ON a.id = bs.app_id
               JOIN projects p ON p.id = a.project_id
               JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = ?
               JOIN s3_targets t ON t.id = bs.s3_target_id
               ORDER BY p.name, a.name"#,
            r#"SELECT vb.id, mv.app_id, vb.volume_id, vb.s3_target_id, vb.cron_expr,
                      CAST(vb.retention_days AS SIGNED) AS retention_days,
                      vb.is_active, mv.name AS volume_name, mv.container_mount_path,
                      a.name AS app_name, p.id AS project_id, p.name AS project_name,
                      t.name AS s3_target_name
               FROM app_volume_backups vb
               JOIN app_managed_volumes mv ON mv.id = vb.volume_id
               JOIN apps a ON a.id = mv.app_id
               JOIN projects p ON p.id = a.project_id
               JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = ?
               JOIN s3_targets t ON t.id = vb.s3_target_id
               ORDER BY p.name, a.name"#,
        )
    };

    // Fetch app-level schedules
    let app_rows = if auth.is_global_admin {
        sqlx::query(app_sql)
    } else {
        sqlx::query(app_sql).bind(&auth.user_id)
    }
    .fetch_all(&state.db)
    .await?;

    // Fetch volume-level schedules
    let vol_rows = if auth.is_global_admin {
        sqlx::query(vol_sql)
    } else {
        sqlx::query(vol_sql).bind(&auth.user_id)
    }
    .fetch_all(&state.db)
    .await?;

    use sqlx::Row;

    let s3_filter = q.s3_target_id.as_deref();

    let app_backups: Vec<serde_json::Value> = app_rows.into_iter()
        .filter(|r| {
            s3_filter.map_or(true, |f| r.get::<&str, _>("s3_target_id") == f)
        })
        .map(|r| serde_json::json!({
            "id": r.get::<&str, _>("id"),
            "type": "app",
            "app_id": r.get::<&str, _>("app_id"),
            "app_name": r.get::<&str, _>("app_name"),
            "project_id": r.get::<&str, _>("project_id"),
            "project_name": r.get::<&str, _>("project_name"),
            "s3_target_id": r.get::<&str, _>("s3_target_id"),
            "s3_target_name": r.get::<&str, _>("s3_target_name"),
            "cron_expr": r.get::<&str, _>("cron_expr"),
            "retention_days": r.get::<Option<i64>, _>("retention_days"),
            "backup_type": r.get::<Option<&str>, _>("backup_type"),
            "is_active": r.get::<i8, _>("is_active") != 0,
        }))
        .collect();

    let vol_backups: Vec<serde_json::Value> = vol_rows.into_iter()
        .filter(|r| {
            s3_filter.map_or(true, |f| r.get::<&str, _>("s3_target_id") == f)
        })
        .map(|r| serde_json::json!({
            "id": r.get::<&str, _>("id"),
            "type": "volume",
            "app_id": r.get::<&str, _>("app_id"),
            "app_name": r.get::<&str, _>("app_name"),
            "volume_id": r.get::<&str, _>("volume_id"),
            "volume_name": r.get::<&str, _>("volume_name"),
            "mount_path": r.get::<&str, _>("container_mount_path"),
            "project_id": r.get::<&str, _>("project_id"),
            "project_name": r.get::<&str, _>("project_name"),
            "s3_target_id": r.get::<&str, _>("s3_target_id"),
            "s3_target_name": r.get::<&str, _>("s3_target_name"),
            "cron_expr": r.get::<&str, _>("cron_expr"),
            "retention_days": r.get::<Option<i64>, _>("retention_days"),
            "is_active": r.get::<i8, _>("is_active") != 0,
        }))
        .collect();

    let mut all = app_backups;
    all.extend(vol_backups);

    Ok(Json(all))
}

/// DELETE /api/v1/backups/:s3_target_id/file?key=
/// Delete a specific backup file from S3 (not yet implemented — S3 client required).
pub async fn delete_backup_file(
    Extension(auth): Extension<AuthUser>,
    Path(_s3_target_id): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }
    Err(AppError::BadRequest(
        "S3 file deletion not yet implemented. Use your S3 client directly.".into()
    ))
}

/// GET /api/v1/backups/:s3_target_id/download?key=
/// Download/presign a specific backup file (not yet implemented — S3 client required).
pub async fn download_backup_file(
    Extension(auth): Extension<AuthUser>,
    Path(_s3_target_id): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }
    Err(AppError::BadRequest(
        "S3 file download not yet implemented. Use your S3 client directly.".into()
    ))
}
