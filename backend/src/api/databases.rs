use axum::{extract::{Path, State}, response::IntoResponse, Extension, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::{auth::middleware::AuthUser, error::{AppError, AppResult}, state::AppState};

pub async fn list(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(project_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;
    let rows = sqlx::query!(
        r#"SELECT di.id, dc.name AS cluster_name, dc.cluster_type,
                  di.db_name, di.db_user, di.k8s_secret_name, di.status, di.created_at
           FROM database_instances di
           JOIN database_clusters dc ON dc.id = di.cluster_id
           WHERE di.project_id = ? ORDER BY di.created_at DESC"#,
        project_id
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(serde_json::json!(rows.iter().map(|r| serde_json::json!({
        "id": r.id,
        "cluster_name": r.cluster_name,
        "cluster_type": r.cluster_type,
        "db_name": r.db_name,
        "db_user": r.db_user,
        "k8s_secret_name": r.k8s_secret_name,
        "status": r.status,
        "created_at": r.created_at,
    })).collect::<Vec<_>>())))
}

pub async fn get(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, db_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;
    let r = sqlx::query!(
        r#"SELECT di.id, dc.name AS cluster_name, dc.cluster_type, dc.host, dc.port,
                  di.db_name, di.db_user, di.k8s_secret_name, di.status
           FROM database_instances di
           JOIN database_clusters dc ON dc.id = di.cluster_id
           WHERE di.id = ? AND di.project_id = ?"#,
        db_id, project_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("database {db_id}")))?;
    Ok(Json(serde_json::json!({
        "id": r.id,
        "cluster_name": r.cluster_name,
        "cluster_type": r.cluster_type,
        "host": r.host,
        "port": r.port,
        "db_name": r.db_name,
        "db_user": r.db_user,
        "k8s_secret_name": r.k8s_secret_name,
        "status": r.status,
    })))
}

#[derive(Deserialize)]
pub struct CreateDbRequest {
    pub cluster_id: String,
    pub name: String,
}

pub async fn create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(project_id): Path<String>,
    Json(body): Json<CreateDbRequest>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;

    let project_name = sqlx::query_scalar!(r#"SELECT name FROM projects WHERE id = ?"#, project_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("project {project_id}")))?;

    let cluster = sqlx::query!(
        r#"SELECT id, cluster_type, host, port, admin_user, admin_password
           FROM database_clusters WHERE id = ? AND is_active = 1"#,
        body.cluster_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("cluster not found or inactive".into()))?;

    // Validate user-supplied name: only allow safe characters for MySQL identifiers
    if body.name.is_empty() || body.name.len() > 32 {
        return Err(AppError::BadRequest("database name must be 1-32 characters".into()));
    }
    if !body.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(AppError::BadRequest(
            "database name may only contain alphanumeric characters and underscores".into(),
        ));
    }

    // Enforce the per-project DB-instance quota — the same gate the template
    // deploy path uses (managed_usage counts database_instances rows directly),
    // so creating a DB directly here can no longer bypass the limit.
    let mut incoming = std::collections::HashMap::new();
    incoming.insert("database_instance".to_string(), 1i64);
    super::managed_usage::check_binding_allowed(&state, &project_id, &incoming).await?;

    let db_name = format!("p_{}_{}", project_name, body.name);
    let db_user = format!(
        "u_{}_{}",
        auth.username.replace(|c: char| !c.is_ascii_alphanumeric() && c != '_', "_"),
        &Uuid::new_v4().to_string()[..6]
    );
    let db_password = generate_password();

    let admin_pass = state.crypto.decrypt(&cluster.admin_password)?;

    crate::k8s::database::provision_database(
        &cluster.cluster_type,
        &cluster.host,
        cluster.port,
        &cluster.admin_user,
        &admin_pass,
        &db_name,
        &db_user,
        &db_password,
    )
    .await?;

    let encrypted_pass = state.crypto.encrypt(&db_password)?;
    let id = Uuid::new_v4().to_string();
    let secret_name = format!("db-{}", &id[..8]);

    sqlx::query!(
        r#"INSERT INTO database_instances
           (id, cluster_id, project_id, created_by, db_name, db_user, db_password, k8s_secret_name)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
        id, body.cluster_id, project_id, auth.user_id,
        db_name, db_user, encrypted_pass, secret_name,
    )
    .execute(&state.db)
    .await?;

    // Create K8s Secret in project namespace (on the correct K8s cluster)
    // Look up which K8s cluster this project deploys to
    let k8s_cluster_id: String = sqlx::query_scalar(
        "SELECT cluster_id FROM projects WHERE id = ? AND cluster_id IS NOT NULL"
    )
    .bind(&project_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Internal("project has no assigned K8s cluster".into()))?;

    crate::k8s::database::create_db_secret(
        &state,
        &k8s_cluster_id,
        &project_name,
        &secret_name,
        &cluster.host,
        cluster.port,
        &db_name,
        &db_user,
        &db_password,
        &cluster.cluster_type,
    )
    .await?;

    Ok((axum::http::StatusCode::CREATED, Json(serde_json::json!({ "id": id, "secret_name": secret_name }))))
}

pub async fn delete_db(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, db_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;

    let row = sqlx::query!(
        r#"SELECT di.db_name, di.db_user, di.k8s_secret_name,
                  dc.cluster_type, dc.host, dc.port, dc.admin_user, dc.admin_password
           FROM database_instances di
           JOIN database_clusters dc ON dc.id = di.cluster_id
           WHERE di.id = ? AND di.project_id = ?"#,
        db_id, project_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("database {db_id}")))?;

    let admin_pass = state.crypto.decrypt(&row.admin_password)?;

    crate::k8s::database::drop_database(
        &row.cluster_type,
        &row.host,
        row.port,
        &row.admin_user,
        &admin_pass,
        &row.db_name,
        &row.db_user,
    )
    .await?;

    // Delete K8s secret (on the correct K8s cluster)
    #[derive(sqlx::FromRow)]
    struct ProjInfo { name: String, cluster_id: Option<String> }
    let proj_info: Option<ProjInfo> = sqlx::query_as(
        "SELECT name, cluster_id FROM projects WHERE id = ?"
    )
    .bind(&project_id)
    .fetch_optional(&state.db)
    .await?;
    let ns = proj_info.as_ref().map(|r| r.name.clone()).unwrap_or_default();
    let k8s_cluster_id = proj_info.and_then(|r| r.cluster_id);
    // row.k8s_secret_name is Option<String> at runtime (sqlx MySQL)
    let secret_name_opt: Option<String> = row.k8s_secret_name
        .map(|v| format!("{v}"));
    if let (Some(secret_name), Some(cid)) = (secret_name_opt, k8s_cluster_id) {
        crate::k8s::database::delete_db_secret(&state, &cid, &ns, &secret_name).await?;
    }

    sqlx::query!(r#"DELETE FROM database_instances WHERE id = ?"#, db_id)
        .execute(&state.db)
        .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn credentials(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, db_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    // OBSERVER cannot see credentials
    let role = sqlx::query_scalar!(
        r#"SELECT role FROM project_members WHERE project_id = ? AND user_id = ?"#,
        project_id, auth.user_id,
    )
    .fetch_optional(&state.db)
    .await?;
    if !auth.is_global_admin && role.as_deref() == Some("OBSERVER") {
        return Err(AppError::Forbidden("OBSERVER cannot view credentials".into()));
    }

    let row = sqlx::query!(
        r#"SELECT di.db_name, di.db_user, di.db_password,
                  dc.host, dc.port, dc.cluster_type
           FROM database_instances di
           JOIN database_clusters dc ON dc.id = di.cluster_id
           WHERE di.id = ? AND di.project_id = ?"#,
        db_id, project_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("database {db_id}")))?;

    let password = state.crypto.decrypt(&row.db_password)?;

    Ok(Json(serde_json::json!({
        "host": row.host,
        "port": row.port,
        "database": row.db_name,
        "username": row.db_user,
        "password": password,
    })))
}

// ─── Admin: DB Clusters ───────────────────────────────────────────────────────

/// Tenant-facing: list active DB clusters available for provisioning.
/// Returns only the safe fields (no admin creds / host).
pub async fn list_clusters_user(
    State(state): State<AppState>,
    Extension(_auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    #[derive(sqlx::FromRow)]
    struct Row { id: String, name: String, cluster_type: String, description: Option<String> }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, name, cluster_type, description \
         FROM database_clusters WHERE is_active = 1 ORDER BY name",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(serde_json::json!(rows.iter().map(|r| serde_json::json!({
        "id": r.id, "name": r.name, "cluster_type": r.cluster_type,
        "description": r.description,
    })).collect::<Vec<_>>())))
}

pub async fn list_clusters(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }
    let rows = sqlx::query!(
        r#"SELECT id, name, cluster_type, host, port, admin_user,
                  max_databases, is_active, description, manager_url
           FROM database_clusters ORDER BY name"#
    ).fetch_all(&state.db).await?;
    Ok(Json(serde_json::json!(rows.iter().map(|r| serde_json::json!({
        "id": r.id, "name": r.name, "cluster_type": r.cluster_type,
        "host": r.host, "port": r.port, "admin_user": r.admin_user,
        "max_databases": r.max_databases, "is_active": r.is_active != 0,
        "description": r.description, "manager_url": r.manager_url,
    })).collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct CreateClusterRequest {
    pub name: String,
    pub cluster_type: String,
    pub host: String,
    pub port: u16,
    pub admin_user: String,
    pub admin_password: String,
    pub max_databases: Option<u32>,
    pub description: Option<String>,
    /// Web-based DB manager URL (e.g. phpMyAdmin, pgAdmin)
    pub manager_url: Option<String>,
}

pub async fn create_cluster(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreateClusterRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }
    let encrypted = state.crypto.encrypt(&body.admin_password)?;
    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO database_clusters
           (id, name, cluster_type, host, port, admin_user, admin_password,
            max_databases, description, manager_url)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        id, body.name, body.cluster_type, body.host, body.port,
        body.admin_user, encrypted, body.max_databases.unwrap_or(0),
        body.description, body.manager_url,
    ).execute(&state.db).await?;
    Ok((axum::http::StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

pub async fn get_cluster(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }
    let r = sqlx::query!(
        r#"SELECT id, name, cluster_type, host, port, admin_user,
                  max_databases, is_active, description, manager_url
           FROM database_clusters WHERE id = ?"#, id
    ).fetch_optional(&state.db).await?.ok_or_else(|| AppError::NotFound(format!("cluster {id}")))?;
    Ok(Json(serde_json::json!({
        "id": r.id, "name": r.name, "cluster_type": r.cluster_type,
        "host": r.host, "port": r.port, "admin_user": r.admin_user,
        "max_databases": r.max_databases, "is_active": r.is_active != 0,
        "description": r.description, "manager_url": r.manager_url,
    })))
}

#[derive(Deserialize)]
pub struct UpdateClusterRequest {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub admin_user: Option<String>,
    pub admin_password: Option<String>,
    pub max_databases: Option<u32>,
    pub description: Option<String>,
    pub manager_url: Option<String>,
    pub is_active: Option<bool>,
}

pub async fn update_cluster(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
    Json(body): Json<UpdateClusterRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let existing = sqlx::query!(
        r#"SELECT host, port, admin_user, max_databases, description, manager_url, is_active
           FROM database_clusters WHERE id = ?"#, id
    ).fetch_optional(&state.db).await?
     .ok_or_else(|| AppError::NotFound(format!("cluster {id}")))?;

    let host = body.host.as_deref().unwrap_or(&existing.host);
    let port = body.port.unwrap_or(existing.port);
    let admin_user = body.admin_user.as_deref().unwrap_or(&existing.admin_user);
    let max_databases = body.max_databases.unwrap_or(existing.max_databases);
    let description = body.description.as_deref().or(existing.description.as_deref());
    let manager_url = body.manager_url.as_deref().or(existing.manager_url.as_deref());
    let is_active = body.is_active.map(|v| v as i8).unwrap_or(existing.is_active);

    if let Some(new_pass) = &body.admin_password {
        let enc = state.crypto.encrypt(new_pass)?;
        sqlx::query!(
            r#"UPDATE database_clusters
               SET host=?, port=?, admin_user=?, admin_password=?,
                   max_databases=?, description=?, manager_url=?, is_active=?
               WHERE id=?"#,
            host, port, admin_user, enc, max_databases, description, manager_url, is_active, id,
        ).execute(&state.db).await?;
    } else {
        sqlx::query!(
            r#"UPDATE database_clusters
               SET host=?, port=?, admin_user=?,
                   max_databases=?, description=?, manager_url=?, is_active=?
               WHERE id=?"#,
            host, port, admin_user, max_databases, description, manager_url, is_active, id,
        ).execute(&state.db).await?;
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn delete_cluster(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }
    let count = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM database_instances WHERE cluster_id = ? AND status = 'ACTIVE'"#, id
    ).fetch_one(&state.db).await?;
    if count > 0 {
        return Err(AppError::Conflict(
            format!("cluster has {count} active databases; drop them first")
        ));
    }
    sqlx::query!(r#"DELETE FROM database_clusters WHERE id = ?"#, id).execute(&state.db).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub(crate) fn generate_password() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..24)
        .map(|_| {
            let idx = rng.gen_range(0..62);
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
                .chars()
                .nth(idx)
                .unwrap()
        })
        .collect()
}
