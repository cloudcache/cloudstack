use axum::{
    extract::{Path, State},
    response::{IntoResponse, Sse},
    Extension, Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::middleware::AuthUser,
    error::{AppError, AppResult},
    state::AppState,
};

// ─── List / Get ───────────────────────────────────────────────────────────────

pub async fn list(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(project_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;

    let rows = sqlx::query!(
        r#"SELECT id, name, display_name, source_type, status,
                  replicas, cpu_limit_mcores, mem_limit_mb,
                  gpu_enabled, gpu_count, created_at
           FROM apps WHERE project_id = ? ORDER BY name"#,
        project_id
    )
    .fetch_all(&state.db)
    .await?;

    let mut apps: Vec<serde_json::Value> = Vec::with_capacity(rows.len());
    for r in rows {
        // Fetch domains and ports per app for the project overview / network graph
        let domains = sqlx::query!(
            r#"SELECT id, hostname, ssl_enabled FROM app_domains WHERE app_id = ?"#, r.id
        ).fetch_all(&state.db).await?;
        let ports = sqlx::query!(
            r#"SELECT id, container_port, protocol FROM app_ports WHERE app_id = ?"#, r.id
        ).fetch_all(&state.db).await?;

        apps.push(serde_json::json!({
            "id": r.id,
            "name": r.name,
            "display_name": r.display_name,
            "source_type": r.source_type,
            "status": r.status,
            "replicas": r.replicas,
            "cpu_limit_mcores": r.cpu_limit_mcores,
            "mem_limit_mb": r.mem_limit_mb,
            "gpu_enabled": r.gpu_enabled != 0,
            "gpu_count": r.gpu_count,
            "created_at": r.created_at,
            "app_domains": domains.iter().map(|d| serde_json::json!({
                "id": d.id, "hostname": d.hostname, "ssl_enabled": d.ssl_enabled != 0,
            })).collect::<Vec<_>>(),
            "app_ports": ports.iter().map(|p| serde_json::json!({
                "id": p.id, "port": p.container_port, "protocol": p.protocol,
            })).collect::<Vec<_>>(),
        }));
    }

    Ok(Json(apps))
}

pub async fn get(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;
    let app = fetch_app(&state, &project_id, &app_id).await?;
    Ok(Json(app))
}

/// GET /api/v1/apps/:app_id — fetch app by ID alone (resolves project internally).
/// Returns same fields as `get` plus `project_id` and `app_domains` array.
pub async fn get_by_id(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(app_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    // Resolve project_id from DB first
    let project_id = sqlx::query_scalar!(r#"SELECT project_id FROM apps WHERE id = ?"#, app_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("app {app_id}")))?;

    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;

    let mut app = fetch_app(&state, &project_id, &app_id).await?;

    // Attach project_id, project_name and app_domains to the response
    app["project_id"] = serde_json::json!(project_id);

    let project_name = sqlx::query_scalar!(r#"SELECT name FROM projects WHERE id = ?"#, project_id)
        .fetch_optional(&state.db)
        .await?
        .unwrap_or_default();
    app["project_name"] = serde_json::json!(project_name);

    let domain_rows = sqlx::query!(
        r#"SELECT id, hostname, ssl_enabled FROM app_domains WHERE app_id = ? ORDER BY created_at"#,
        app_id
    )
    .fetch_all(&state.db)
    .await?;

    app["app_domains"] = serde_json::json!(domain_rows
        .iter()
        .map(|d| serde_json::json!({
            "id": d.id,
            "hostname": d.hostname,
            "ssl_enabled": d.ssl_enabled != 0,
        }))
        .collect::<Vec<_>>());

    // Include related collections needed by the frontend
    let port_rows = sqlx::query!(
        r#"SELECT id, container_port, protocol, nodeport FROM app_ports WHERE app_id = ?"#,
        app_id
    ).fetch_all(&state.db).await?;
    app["app_ports"] = serde_json::json!(port_rows.iter().map(|p| serde_json::json!({
        "id": p.id, "port": p.container_port, "protocol": p.protocol, "nodeport": p.nodeport,
    })).collect::<Vec<_>>());

    let volume_rows = sqlx::query!(
        r#"SELECT id, name, container_mount_path, host_path, share_with_others, shared_volume_id
           FROM app_managed_volumes WHERE app_id = ?"#,
        app_id
    ).fetch_all(&state.db).await?;
    app["app_volumes"] = serde_json::json!(volume_rows.iter().map(|v| serde_json::json!({
        "id": v.id, "name": v.name, "mountPath": v.container_mount_path, "hostPath": v.host_path,
        "shareWithOthers": v.share_with_others != 0, "sharedVolumeId": v.shared_volume_id,
    })).collect::<Vec<_>>());

    let fm_rows = sqlx::query!(
        r#"SELECT id, filename, mount_path FROM app_file_mounts WHERE app_id = ? ORDER BY mount_path, filename"#,
        app_id
    ).fetch_all(&state.db).await?;
    app["app_file_mounts"] = serde_json::json!(fm_rows.iter().map(|f| serde_json::json!({
        "id": f.id, "filename": f.filename, "mountPath": f.mount_path,
    })).collect::<Vec<_>>());

    let ba_rows = sqlx::query!(
        r#"SELECT id, username FROM app_basic_auth WHERE app_id = ?"#,
        app_id
    ).fetch_all(&state.db).await?;
    app["app_basic_auths"] = serde_json::json!(ba_rows.iter().map(|b| serde_json::json!({
        "id": b.id, "username": b.username,
    })).collect::<Vec<_>>());

    Ok(Json(app))
}

// ─── Create ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateAppRequest {
    pub name: String,
    pub display_name: Option<String>,
    pub pool_id: Option<String>,
    pub source_type: String,

    // Container source
    pub container_image: Option<String>,
    pub container_registry_user: Option<String>,
    pub container_registry_pass: Option<String>,

    // Git source
    pub git_url: Option<String>,
    pub git_branch: Option<String>,
    pub git_token: Option<String>,
    pub dockerfile_path: Option<String>,

    // Runtime
    pub container_command: Option<String>,
    pub container_args: Option<Vec<String>>,
    pub working_dir: Option<String>,
    pub replicas: Option<u8>,

    // Resources
    pub cpu_reservation_mcores: Option<u32>,
    pub cpu_limit_mcores: Option<u32>,
    pub mem_reservation_mb: Option<u32>,
    pub mem_limit_mb: Option<u32>,

    // Security context
    pub run_as_user: Option<u32>,
    pub run_as_group: Option<u32>,
    pub fs_group: Option<u32>,
    pub privileged: Option<bool>,
    pub read_only_root_fs: Option<bool>,

    // GPU
    pub gpu_enabled: Option<bool>,
    pub gpu_count: Option<u8>,

    // Mounts
    pub mount_ldap_files: Option<bool>,
    pub mount_etc_hosts: Option<bool>,
    pub mount_user_home: Option<bool>,
    pub mount_app_data: Option<bool>,
    pub mount_app_logs: Option<bool>,
    pub timezone: Option<String>,

    // Scheduling
    pub anti_affinity_enabled: Option<bool>,

    // Health checks
    pub health_check_type: Option<String>,
    pub health_check_path: Option<String>,
    pub health_check_port: Option<u16>,
    pub health_check_scheme: Option<String>,
    pub health_check_period: Option<u16>,
    pub health_check_timeout: Option<u16>,
    pub health_check_failures: Option<u8>,

    // App type + network policy
    pub app_type: Option<String>,
    pub use_network_policy: Option<bool>,
    pub ingress_network_policy: Option<String>,
    pub egress_network_policy: Option<String>,
}

pub async fn create(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(project_id): Path<String>,
    Json(body): Json<CreateAppRequest>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;

    // Quota: app count check
    let existing = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM apps WHERE project_id = ?"#,
        project_id
    )
    .fetch_one(&state.db)
    .await?;

    let project_quota = sqlx::query_scalar!(
        r#"SELECT quota_apps FROM projects WHERE id = ?"#,
        project_id
    )
    .fetch_one(&state.db)
    .await?;

    if project_quota > 0 && existing >= project_quota as i64 {
        return Err(AppError::QuotaExceeded("project app quota reached".into()));
    }

    let reg_pass = body
        .container_registry_pass
        .as_deref()
        .map(|p| state.crypto.encrypt(p))
        .transpose()?;

    let git_token = body
        .git_token
        .as_deref()
        .map(|t| state.crypto.encrypt(t))
        .transpose()?;

    let container_args_json = body
        .container_args
        .as_ref()
        .map(|v| serde_json::to_string(v))
        .transpose()
        .map_err(|e| AppError::BadRequest(format!("invalid container_args: {e}")))?;

    let id = Uuid::new_v4().to_string();
    let webhook_id = Uuid::new_v4().to_string();
    let resolved_pool_id = match body.pool_id.as_deref() {
        Some(pool_id) => Some(pool_id.to_string()),
        None => default_active_pool_id(&state).await?,
    };

    let health_check_type = body.health_check_type.as_deref().unwrap_or("NONE");
    let health_check_scheme = body.health_check_scheme.as_deref().unwrap_or("HTTP");
    let app_type = body.app_type.as_deref().unwrap_or("APP");
    let ingress_network_policy = body
        .ingress_network_policy
        .as_deref()
        .unwrap_or("ALLOW_ALL");
    let egress_network_policy = body.egress_network_policy.as_deref().unwrap_or("ALLOW_ALL");

    sqlx::query!(
        r#"INSERT INTO apps
           (id, project_id, pool_id, name, display_name, owner_id, source_type,
            container_image, container_registry_user, container_registry_pass,
            git_url, git_branch, git_token, dockerfile_path,
            container_command, container_args, working_dir,
            replicas, cpu_reservation_mcores, cpu_limit_mcores,
            mem_reservation_mb, mem_limit_mb,
            run_as_user, run_as_group, fs_group,
            privileged, read_only_root_fs,
            gpu_enabled, gpu_count, timezone,
            mount_ldap_files, mount_etc_hosts, mount_user_home, mount_app_data, mount_app_logs,
            anti_affinity_enabled,
            health_check_type, health_check_path, health_check_port,
            health_check_scheme, health_check_period, health_check_timeout, health_check_failures,
            webhook_id,
            app_type, use_network_policy, ingress_network_policy, egress_network_policy)
           VALUES (?, ?, ?, ?, ?, ?, ?,
                   ?, ?, ?,
                   ?, ?, ?, ?,
                   ?, ?, ?,
                   ?, ?, ?,
                   ?, ?,
                   ?, ?, ?,
                   ?, ?,
                   ?, ?, ?,
                   ?, ?, ?, ?, ?,
                   ?,
                   ?, ?, ?,
                   ?, ?, ?, ?,
                   ?,
                   ?, ?, ?, ?)"#,
        id,
        project_id,
        resolved_pool_id,
        body.name,
        body.display_name,
        auth.user_id,
        body.source_type,
        body.container_image,
        body.container_registry_user,
        reg_pass,
        body.git_url,
        body.git_branch,
        git_token,
        body.dockerfile_path.as_deref().unwrap_or("Dockerfile"),
        body.container_command,
        container_args_json,
        body.working_dir,
        body.replicas.unwrap_or(1),
        body.cpu_reservation_mcores,
        body.cpu_limit_mcores,
        body.mem_reservation_mb,
        body.mem_limit_mb,
        body.run_as_user,
        body.run_as_group,
        body.fs_group,
        body.privileged.unwrap_or(false) as i8,
        body.read_only_root_fs.unwrap_or(false) as i8,
        body.gpu_enabled.unwrap_or(false) as i8,
        body.gpu_count.unwrap_or(0),
        body.timezone.as_deref().unwrap_or("Asia/Shanghai"),
        body.mount_ldap_files.unwrap_or(true) as i8,
        body.mount_etc_hosts.unwrap_or(true) as i8,
        body.mount_user_home.unwrap_or(true) as i8,
        body.mount_app_data.unwrap_or(true) as i8,
        body.mount_app_logs.unwrap_or(true) as i8,
        body.anti_affinity_enabled.unwrap_or(true) as i8,
        health_check_type,
        body.health_check_path,
        body.health_check_port,
        health_check_scheme,
        body.health_check_period.unwrap_or(10),
        body.health_check_timeout.unwrap_or(5),
        body.health_check_failures.unwrap_or(3),
        webhook_id,
        app_type,
        body.use_network_policy.unwrap_or(false) as i8,
        ingress_network_policy,
        egress_network_policy,
    )
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref de) if de.is_unique_violation() => AppError::Conflict(format!(
            "app name '{}' already exists in project",
            body.name
        )),
        other => AppError::Database(other),
    })?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "id": id, "webhook_id": webhook_id })),
    ))
}

// ─── Update ───────────────────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct UpdateAppRequest {
    pub display_name: Option<String>,
    pub pool_id: Option<String>,

    // Container source
    pub container_image: Option<String>,
    pub container_registry_user: Option<String>,
    pub container_registry_pass: Option<String>,

    // Git source
    pub git_url: Option<String>,
    pub git_branch: Option<String>,
    pub git_token: Option<String>,
    pub dockerfile_path: Option<String>,

    // Runtime
    pub container_command: Option<String>,
    pub container_args: Option<Vec<String>>,
    pub working_dir: Option<String>,

    // Resources
    pub cpu_reservation_mcores: Option<u32>,
    pub cpu_limit_mcores: Option<u32>,
    pub mem_reservation_mb: Option<u32>,
    pub mem_limit_mb: Option<u32>,

    // Security context
    pub run_as_user: Option<u32>,
    pub run_as_group: Option<u32>,
    pub fs_group: Option<u32>,
    pub privileged: Option<bool>,
    pub read_only_root_fs: Option<bool>,

    // GPU
    pub gpu_enabled: Option<bool>,
    pub gpu_count: Option<u8>,

    // Mounts
    pub mount_ldap_files: Option<bool>,
    pub mount_etc_hosts: Option<bool>,
    pub mount_user_home: Option<bool>,
    pub mount_app_data: Option<bool>,
    pub mount_app_logs: Option<bool>,
    pub timezone: Option<String>,

    // Scheduling
    pub anti_affinity_enabled: Option<bool>,

    // Health checks
    pub health_check_type: Option<String>,
    pub health_check_path: Option<String>,
    pub health_check_port: Option<u16>,
    pub health_check_scheme: Option<String>,
    pub health_check_period: Option<u16>,
    pub health_check_timeout: Option<u16>,
    pub health_check_failures: Option<u8>,

    // App type + network policy
    pub app_type: Option<String>,
    pub use_network_policy: Option<bool>,
    pub ingress_network_policy: Option<String>,
    pub egress_network_policy: Option<String>,
}

pub async fn update(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
    Json(body): Json<UpdateAppRequest>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;

    // Verify app belongs to project
    let exists = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM apps WHERE id = ? AND project_id = ?"#,
        app_id,
        project_id
    )
    .fetch_one(&state.db)
    .await?;
    if exists == 0 {
        return Err(AppError::NotFound(format!("app {app_id}")));
    }

    // Re-encrypt secrets if provided
    let reg_pass = body
        .container_registry_pass
        .as_deref()
        .map(|p| state.crypto.encrypt(p))
        .transpose()?;
    let git_token = body
        .git_token
        .as_deref()
        .map(|t| state.crypto.encrypt(t))
        .transpose()?;
    let container_args_json = body
        .container_args
        .as_ref()
        .map(|v| serde_json::to_string(v))
        .transpose()
        .map_err(|e| AppError::BadRequest(format!("invalid container_args: {e}")))?;

    // Build a single UPDATE with only the provided fields using runtime query building.
    // We use sqlx::query (not query!) to avoid compile-time DB requirement for dynamic SQL.
    let mut sets: Vec<&str> = Vec::new();
    let mut str_vals: Vec<String> = Vec::new();

    // Helper: push a &str SET clause + owned value
    macro_rules! push_str {
        ($col:expr, $val:expr) => {
            if let Some(v) = $val {
                sets.push($col);
                str_vals.push(v.to_string());
            }
        };
    }

    push_str!("display_name = ?", body.display_name.as_deref());
    push_str!("pool_id = ?", body.pool_id.as_deref());
    push_str!("container_image = ?", body.container_image.as_deref());
    push_str!(
        "container_registry_user = ?",
        body.container_registry_user.as_deref()
    );
    push_str!("container_registry_pass = ?", reg_pass.as_deref());
    push_str!("git_url = ?", body.git_url.as_deref());
    push_str!("git_branch = ?", body.git_branch.as_deref());
    push_str!("git_token = ?", git_token.as_deref());
    push_str!("dockerfile_path = ?", body.dockerfile_path.as_deref());
    push_str!("container_command = ?", body.container_command.as_deref());
    push_str!("container_args = ?", container_args_json.as_deref());
    push_str!("working_dir = ?", body.working_dir.as_deref());
    push_str!("timezone = ?", body.timezone.as_deref());
    push_str!("health_check_type = ?", body.health_check_type.as_deref());
    push_str!("health_check_path = ?", body.health_check_path.as_deref());
    push_str!(
        "health_check_scheme = ?",
        body.health_check_scheme.as_deref()
    );
    push_str!("app_type = ?", body.app_type.as_deref());
    push_str!(
        "ingress_network_policy = ?",
        body.ingress_network_policy.as_deref()
    );
    push_str!(
        "egress_network_policy = ?",
        body.egress_network_policy.as_deref()
    );

    // Numeric / bool fields stored as strings for the unified binder
    macro_rules! push_num {
        ($col:expr, $val:expr) => {
            if let Some(v) = $val {
                sets.push($col);
                str_vals.push(v.to_string());
            }
        };
    }
    push_num!("cpu_reservation_mcores = ?", body.cpu_reservation_mcores);
    push_num!("cpu_limit_mcores = ?", body.cpu_limit_mcores);
    push_num!("mem_reservation_mb = ?", body.mem_reservation_mb);
    push_num!("mem_limit_mb = ?", body.mem_limit_mb);
    push_num!("run_as_user = ?", body.run_as_user);
    push_num!("run_as_group = ?", body.run_as_group);
    push_num!("fs_group = ?", body.fs_group);
    push_num!("privileged = ?", body.privileged.map(|v| v as u8));
    push_num!(
        "read_only_root_fs = ?",
        body.read_only_root_fs.map(|v| v as u8)
    );
    push_num!("gpu_enabled = ?", body.gpu_enabled.map(|v| v as u8));
    push_num!("gpu_count = ?", body.gpu_count);
    push_num!(
        "mount_ldap_files = ?",
        body.mount_ldap_files.map(|v| v as u8)
    );
    push_num!("mount_etc_hosts = ?", body.mount_etc_hosts.map(|v| v as u8));
    push_num!("mount_user_home = ?", body.mount_user_home.map(|v| v as u8));
    push_num!("mount_app_data = ?", body.mount_app_data.map(|v| v as u8));
    push_num!("mount_app_logs = ?", body.mount_app_logs.map(|v| v as u8));
    push_num!(
        "anti_affinity_enabled = ?",
        body.anti_affinity_enabled.map(|v| v as u8)
    );
    push_num!("health_check_port = ?", body.health_check_port);
    push_num!("health_check_period = ?", body.health_check_period);
    push_num!("health_check_timeout = ?", body.health_check_timeout);
    push_num!("health_check_failures = ?", body.health_check_failures);
    push_num!(
        "use_network_policy = ?",
        body.use_network_policy.map(|v| v as u8)
    );

    if sets.is_empty() {
        return Ok(axum::http::StatusCode::NO_CONTENT);
    }

    let sql = format!("UPDATE apps SET {} WHERE id = ?", sets.join(", "));
    let mut q = sqlx::query(&sql);
    for v in &str_vals {
        q = q.bind(v);
    }
    q = q.bind(&app_id);
    q.execute(&state.db).await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn delete_app(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;

    let app = fetch_app_basic(&state, &project_id, &app_id).await?;
    let ns = project_namespace(&state, &project_id).await?;

    if let Ok(cluster_id) = resolve_cluster_for_app(&state, &app_id).await {
        let orch = cluster_orchestrator(&state, &cluster_id).await.unwrap_or_else(|_| "K3S".into());
        if orch == "DOCKER" {
            let _ = crate::docker::deployment::delete_app_resources(
                &state, &cluster_id, &ns, &app.name, &app_id,
            ).await;
        } else {
            let _ = crate::k8s::deployment::delete_app_resources(
                &state, &cluster_id, &ns, &app.name, &app_id,
            ).await;
        }
    }

    // Release fixed IP allocations from VPC / public pools
    let _ = crate::k8s::network::release_app_ips(&state, &app_id).await;

    // Drop managed-service resources this app provisioned via templates.
    // Best-effort: errors are logged inside the helper, app delete proceeds.
    let _ = crate::api::templates_deploy::cleanup_bindings_for_app(&state, &app_id).await;

    sqlx::query!(
        r#"DELETE FROM apps WHERE id = ? AND project_id = ?"#,
        app_id,
        project_id
    )
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Deploy ───────────────────────────────────────────────────────────────────

pub async fn deploy(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;

    let app_res = sqlx::query!(
        r#"SELECT replicas,
                  COALESCE(cpu_limit_mcores, cpu_reservation_mcores, 0) AS cpu,
                  COALESCE(mem_limit_mb, mem_reservation_mb, 0) AS mem,
                  status
           FROM apps WHERE id = ? AND project_id = ?"#,
        app_id,
        project_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("app {app_id}")))?;

    // Only charge delta for apps currently holding no resources
    let currently_free = matches!(app_res.status.as_str(), "STOPPED" | "SUSPENDED" | "FAILED");
    if currently_free {
        let extra_cpu = app_res.cpu as i64 * app_res.replicas as i64;
        let extra_mem = app_res.mem as i64 * app_res.replicas as i64;
        crate::quota::check_deploy_allowed(&state, &project_id, extra_cpu, extra_mem, 1).await?;
    }

    let quota_status = crate::quota::check(&state, &project_id).await?;
    if quota_status.any_warned() {
        crate::quota::record_violation(
            &state,
            &project_id,
            Some(&app_id),
            "cpu_mcores",
            quota_status.usage.cpu_mcores,
            quota_status.quota.cpu_mcores,
            quota_status.cpu_pct,
            "warn",
        )
        .await;
        tracing::warn!(
            project_id,
            app_id,
            cpu_pct = quota_status.cpu_pct,
            mem_pct = quota_status.mem_pct,
            "quota approaching limit"
        );
    }

    let cluster_id = resolve_cluster_for_app(&state, &app_id).await?;

    super::events::record(&state, &app_id, "DEPLOY", "PENDING", &auth.user_id, None).await;

    let orch = cluster_orchestrator(&state, &cluster_id).await?;
    let state_clone = state.clone();
    let app_id_clone = app_id.clone();
    let project_id_clone = project_id.clone();
    let user_id = auth.user_id.clone();
    tokio::spawn(async move {
        let result = if orch == "DOCKER" {
            crate::docker::deployment::deploy_app(
                &state_clone,
                &cluster_id,
                &project_id_clone,
                &app_id_clone,
                &user_id,
            )
            .await
        } else {
            crate::k8s::deployment::deploy_app(
                &state_clone,
                &cluster_id,
                &project_id_clone,
                &app_id_clone,
                &user_id,
            )
            .await
        };
        match result {
            Ok(_) => {
                super::events::record(
                    &state_clone,
                    &app_id_clone,
                    "DEPLOY",
                    "RUNNING",
                    &user_id,
                    None,
                )
                .await;
            }
            Err(e) => {
                tracing::error!("deploy failed for app {app_id_clone}: {e}");
                super::events::record(
                    &state_clone,
                    &app_id_clone,
                    "DEPLOY",
                    "FAILED",
                    &user_id,
                    Some(&e.to_string()),
                )
                .await;
                let _ = sqlx::query!(
                    r#"UPDATE apps SET status = 'FAILED' WHERE id = ?"#,
                    app_id_clone
                )
                .execute(&state_clone.db)
                .await;
            }
        }
    });

    Ok(Json(serde_json::json!({ "status": "deploying" })))
}

// ─── Pause / Resume ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PauseRequest {
    pub reason: Option<String>,
}

pub async fn pause(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
    Json(body): Json<PauseRequest>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    let app = fetch_app_basic(&state, &project_id, &app_id).await?;

    let ns = project_namespace(&state, &project_id).await?;
    let cluster_id = resolve_cluster_for_app(&state, &app_id).await?;
    let orch = cluster_orchestrator(&state, &cluster_id).await?;
    if orch == "DOCKER" {
        crate::docker::deployment::scale_deployment(&state, &cluster_id, &ns, &app.name, &app_id, 0).await?;
    } else {
        crate::k8s::deployment::scale_deployment(&state, &cluster_id, &ns, &app.name, 0).await?;
    }

    // Set pingora maintenance location for all domains
    for domain in app_domains(&state, &app_id).await? {
        if let Some(pingora) = state.pingora.read().await.as_ref() {
            let _ = pingora.set_maintenance_location(&domain).await;
        }
    }

    sqlx::query!(
        r#"UPDATE apps
           SET status = 'PAUSED', paused_at = NOW(), paused_by = ?, pause_reason = ?
           WHERE id = ?"#,
        auth.user_id,
        body.reason,
        app_id,
    )
    .execute(&state.db)
    .await?;

    super::events::record(
        &state,
        &app_id,
        "PAUSE",
        "SUCCEEDED",
        &auth.user_id,
        body.reason.as_deref(),
    )
    .await;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn resume(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    let app = fetch_app_basic(&state, &project_id, &app_id).await?;

    // SUSPENDED apps require quota headroom before resuming
    if app.status == "SUSPENDED" {
        let res = sqlx::query!(
            r#"SELECT COALESCE(cpu_limit_mcores, cpu_reservation_mcores, 0) AS cpu,
                      COALESCE(mem_limit_mb, mem_reservation_mb, 0) AS mem
               FROM apps WHERE id = ?"#,
            app_id
        )
        .fetch_one(&state.db)
        .await?;
        let extra_cpu = res.cpu as i64 * app.replicas as i64;
        let extra_mem = res.mem as i64 * app.replicas as i64;
        crate::quota::check_deploy_allowed(&state, &project_id, extra_cpu, extra_mem, 0).await?;
    }

    let ns = project_namespace(&state, &project_id).await?;
    let cluster_id = resolve_cluster_for_app(&state, &app_id).await?;
    let orch = cluster_orchestrator(&state, &cluster_id).await?;
    if orch == "DOCKER" {
        crate::docker::deployment::scale_deployment(
            &state, &cluster_id, &ns, &app.name, &app_id, app.replicas as i32,
        ).await?;
    } else {
        crate::k8s::deployment::scale_deployment(
            &state, &cluster_id, &ns, &app.name, app.replicas as i32,
        ).await?;
    }

    // Remove pingora maintenance locations
    for domain in app_domains(&state, &app_id).await? {
        if let Some(pingora) = state.pingora.read().await.as_ref() {
            let _ = pingora.remove_maintenance_location(&domain).await;
        }
    }

    sqlx::query!(
        r#"UPDATE apps
           SET status = 'RUNNING', paused_at = NULL, paused_by = NULL, pause_reason = NULL
           WHERE id = ?"#,
        app_id,
    )
    .execute(&state.db)
    .await?;

    super::events::record(&state, &app_id, "RESUME", "SUCCEEDED", &auth.user_id, None).await;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Scale ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ScaleRequest {
    pub replicas: i32,
}

pub async fn scale(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
    Json(body): Json<ScaleRequest>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    let app = fetch_app_basic(&state, &project_id, &app_id).await?;

    if body.replicas < 0 {
        return Err(AppError::BadRequest("replicas must be >= 0".into()));
    }

    // If scaling up, check quota for the extra replicas
    let delta_replicas = body.replicas - app.replicas as i32;
    if delta_replicas > 0 {
        let res = sqlx::query!(
            r#"SELECT COALESCE(cpu_limit_mcores, cpu_reservation_mcores, 0) AS cpu,
                      COALESCE(mem_limit_mb, mem_reservation_mb, 0) AS mem
               FROM apps WHERE id = ?"#,
            app_id
        )
        .fetch_one(&state.db)
        .await?;
        let extra_cpu = res.cpu as i64 * delta_replicas as i64;
        let extra_mem = res.mem as i64 * delta_replicas as i64;
        crate::quota::check_deploy_allowed(&state, &project_id, extra_cpu, extra_mem, 0).await?;
    }

    let ns = project_namespace(&state, &project_id).await?;
    let cluster_id = resolve_cluster_for_app(&state, &app_id).await?;
    let orch = cluster_orchestrator(&state, &cluster_id).await?;

    if orch == "DOCKER" {
        crate::docker::deployment::scale_deployment(&state, &cluster_id, &ns, &app.name, &app_id, body.replicas)
            .await?;
    } else {
        crate::k8s::deployment::scale_deployment(&state, &cluster_id, &ns, &app.name, body.replicas)
            .await?;
    }

    sqlx::query!(
        r#"UPDATE apps SET replicas = ? WHERE id = ?"#,
        body.replicas,
        app_id,
    )
    .execute(&state.db)
    .await?;

    super::events::record(
        &state,
        &app_id,
        "SCALE",
        "SUCCEEDED",
        &auth.user_id,
        Some(&format!("scaled to {} replicas", body.replicas)),
    )
    .await;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Env vars ─────────────────────────────────────────────────────────────────

pub async fn list_env(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;

    let rows = sqlx::query!(
        r#"SELECT id, key_name, value, is_secret
           FROM app_env_vars WHERE app_id = ? ORDER BY key_name"#,
        app_id
    )
    .fetch_all(&state.db)
    .await?;

    let is_observer = !auth.is_global_admin
        && sqlx::query_scalar!(
            r#"SELECT role FROM project_members WHERE project_id = ? AND user_id = ?"#,
            project_id,
            auth.user_id,
        )
        .fetch_optional(&state.db)
        .await?
        .map(|r: String| r == "OBSERVER")
        .unwrap_or(false);

    let vars: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            let value = if r.is_secret != 0 && is_observer {
                None
            } else if r.is_secret != 0 {
                r.value
                    .as_deref()
                    .and_then(|v| state.crypto.decrypt(v).ok())
            } else {
                r.value.clone()
            };
            serde_json::json!({
                "id": r.id,
                "key": r.key_name,
                "value": value,
                "is_secret": r.is_secret != 0,
            })
        })
        .collect();

    Ok(Json(vars))
}

#[derive(Deserialize)]
pub struct SetEnvRequest {
    pub key: String,
    pub value: String,
    pub is_secret: Option<bool>,
}

pub async fn set_env(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
    Json(body): Json<SetEnvRequest>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    let is_secret = body.is_secret.unwrap_or(false);
    let stored_value = if is_secret {
        state.crypto.encrypt(&body.value)?
    } else {
        body.value.clone()
    };

    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO app_env_vars (id, app_id, key_name, value, is_secret)
           VALUES (?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE value = ?, is_secret = ?"#,
        id,
        app_id,
        body.key,
        stored_value,
        is_secret as i8,
        stored_value,
        is_secret as i8,
    )
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn delete_env(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, _app_id, env_id)): Path<(String, String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    sqlx::query!(r#"DELETE FROM app_env_vars WHERE id = ?"#, env_id)
        .execute(&state.db)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Ports ────────────────────────────────────────────────────────────────────

pub async fn list_ports(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;
    let rows = sqlx::query!(
        r#"SELECT id, container_port, protocol, nodeport FROM app_ports WHERE app_id = ?"#,
        app_id
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(serde_json::json!(rows
        .iter()
        .map(|r| serde_json::json!({
            "id": r.id,
            "container_port": r.container_port,
            "protocol": r.protocol,
            "nodeport": r.nodeport,
        }))
        .collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct AddPortRequest {
    pub container_port: u16,
    pub protocol: Option<String>,
}

pub async fn add_port(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
    Json(body): Json<AddPortRequest>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO app_ports (id, app_id, container_port, protocol)
           VALUES (?, ?, ?, ?)"#,
        id,
        app_id,
        body.container_port,
        body.protocol.as_deref().unwrap_or("TCP"),
    )
    .execute(&state.db)
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "id": id })),
    ))
}

pub async fn delete_port(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, _app_id, port_id)): Path<(String, String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    sqlx::query!(r#"DELETE FROM app_ports WHERE id = ?"#, port_id)
        .execute(&state.db)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Logs (SSE) ───────────────────────────────────────────────────────────────

pub async fn logs_stream(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;
    let app = fetch_app_basic(&state, &project_id, &app_id).await?;
    let ns = project_namespace(&state, &project_id).await?;

    let cluster_id = resolve_cluster_for_app(&state, &app_id).await?;
    let orch = cluster_orchestrator(&state, &cluster_id).await?;
    if orch == "DOCKER" {
        let stream = crate::docker::deployment::log_stream(&state, &cluster_id, &ns, &app.name, &app_id).await?;
        Ok(Sse::new(stream).into_response())
    } else {
        let stream = crate::k8s::deployment::log_stream(&state, &cluster_id, &ns, &app.name).await?;
        Ok(Sse::new(stream).into_response())
    }
}

// ─── Terminal (WebSocket) ─────────────────────────────────────────────────────

pub async fn terminal_ws(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
    ws: axum::extract::WebSocketUpgrade,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    let app = fetch_app_basic(&state, &project_id, &app_id).await?;
    let ns = project_namespace(&state, &project_id).await?;

    let cluster_id = resolve_cluster_for_app(&state, &app_id).await?;
    let orch = cluster_orchestrator(&state, &cluster_id).await?;
    let app_id_clone = app_id.clone();
    Ok(ws.on_upgrade(move |socket| async move {
        if orch == "DOCKER" {
            crate::docker::deployment::handle_terminal(socket, state, cluster_id, ns, app.name, app_id_clone).await;
        } else {
            crate::k8s::deployment::handle_terminal(socket, state, cluster_id, ns, app.name).await;
        }
    }))
}

// ─── Metrics ─────────────────────────────────────────────────────────────────

pub async fn metrics_current(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;

    use crate::metrics::names::*;

    let metrics_to_fetch: &[&str] = &[
        APP_CPU_USED_MCORES,
        APP_MEM_USED_BYTES,
        APP_DISK_READ_BYTES_RATE,
        APP_DISK_WRITE_BYTES_RATE,
        APP_NET_RX_BYTES_RATE,
        APP_NET_TX_BYTES_RATE,
        APP_GPU_UTIL_PCT,
        APP_GPU_MEM_USED_BYTES,
        APP_POD_COUNT,
    ];

    let mut result = serde_json::Map::new();
    for &metric_name in metrics_to_fetch {
        let sel =
            crate::metrics::types::MetricSelector::new(metric_name).label("app_id", app_id.clone());
        let pts = state.metrics.query_latest(sel).await.unwrap_or_default();
        let values: Vec<serde_json::Value> = pts
            .iter()
            .map(|p| {
                let mut obj = serde_json::Map::new();
                obj.insert("value".into(), serde_json::json!(p.value));
                obj.insert("timestamp".into(), serde_json::json!(p.timestamp));
                for (k, v) in &p.labels {
                    obj.insert(k.to_string(), serde_json::json!(v));
                }
                serde_json::Value::Object(obj)
            })
            .collect();
        result.insert(metric_name.to_string(), serde_json::json!(values));
    }

    Ok(Json(serde_json::Value::Object(result)))
}

#[derive(Deserialize)]
pub struct MetricsHistoryQuery {
    pub metric: String,
    pub range: Option<String>,
    pub step: Option<u32>,
}

pub async fn metrics_history(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
    axum::extract::Query(q): axum::extract::Query<MetricsHistoryQuery>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;

    let now = chrono::Utc::now().timestamp();
    let range_secs: i64 = match q.range.as_deref().unwrap_or("1h") {
        "30m" => 1_800,
        "1h" => 3_600,
        "6h" => 21_600,
        "24h" => 86_400,
        "7d" => 604_800,
        other => other.parse().unwrap_or(3_600),
    };
    let step = q.step.unwrap_or(60);
    let start = now - range_secs;

    let sel = crate::metrics::types::MetricSelector::new(q.metric.clone())
        .label("app_id", app_id.clone());

    let series = state
        .metrics
        .query_range(sel, start, now, step)
        .await
        .unwrap_or_default();

    Ok(Json(serde_json::json!({
        "metric": q.metric,
        "app_id": app_id,
        "start": start,
        "end": now,
        "step_secs": step,
        "series": series.iter().map(|s| serde_json::json!({
            "labels": s.labels,
            "data": s.points,
        })).collect::<Vec<_>>(),
    })))
}

// ─── Webhook ─────────────────────────────────────────────────────────────────

pub async fn webhook(
    State(state): State<AppState>,
    Path(webhook_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let row = sqlx::query!(
        r#"SELECT id, project_id FROM apps WHERE webhook_id = ?"#,
        webhook_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("webhook not found".into()))?;

    let cluster_id = resolve_cluster_for_app(&state, &row.id).await?;
    let orch = cluster_orchestrator(&state, &cluster_id).await?;
    let state_clone = state.clone();
    tokio::spawn(async move {
        let result = if orch == "DOCKER" {
            crate::docker::deployment::deploy_app(
                &state_clone, &cluster_id, &row.project_id, &row.id, "webhook",
            ).await
        } else {
            crate::k8s::deployment::deploy_app(
                &state_clone, &cluster_id, &row.project_id, &row.id, "webhook",
            ).await
        };
        if let Err(e) = result {
            tracing::error!("webhook deploy failed: {e}");
            let _ = sqlx::query!(r#"UPDATE apps SET status = 'FAILED' WHERE id = ?"#, row.id)
                .execute(&state_clone.db)
                .await;
        }
    });

    Ok(Json(serde_json::json!({ "status": "queued" })))
}

// ─── Pods ─────────────────────────────────────────────────────────────────────

pub async fn list_pods(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;

    // Verify app belongs to project
    let app_name = sqlx::query_scalar!(
        r#"SELECT name FROM apps WHERE id = ? AND project_id = ?"#,
        app_id,
        project_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("app {app_id}")))?;

    let cluster_id = resolve_cluster_for_app(&state, &app_id).await?;
    let orch = cluster_orchestrator(&state, &cluster_id).await?;

    if orch == "DOCKER" {
        let containers = crate::docker::deployment::list_containers(&state, &app_id)
            .await
            .unwrap_or_default();
        Ok(Json(containers))
    } else {
        let ns = project_namespace(&state, &project_id).await?;
        let pods = crate::k8s::pod_spec::list_pods_for_app(&state, &cluster_id, &ns, &app_name)
            .await
            .unwrap_or_default();
        Ok(Json(pods))
    }
}

// ─── Webhook regenerate ───────────────────────────────────────────────────────

pub async fn regenerate_webhook(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;

    let new_webhook_id = uuid::Uuid::new_v4().to_string();
    sqlx::query!(
        r#"UPDATE apps SET webhook_id = ? WHERE id = ? AND project_id = ?"#,
        new_webhook_id,
        app_id,
        project_id
    )
    .execute(&state.db)
    .await?;

    Ok(Json(serde_json::json!({ "webhook_id": new_webhook_id })))
}

// ─── Deployment history ───────────────────────────────────────────────────────

pub async fn deployment_history(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;

    let rows = sqlx::query!(
        r#"SELECT id, event_type, status, triggered_by, message, created_at
           FROM deployment_events WHERE app_id = ?
           ORDER BY created_at DESC LIMIT 100"#,
        app_id
    )
    .fetch_all(&state.db)
    .await?;

    let events: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "event_type": r.event_type,
                "status": r.status,
                "triggered_by": r.triggered_by,
                "message": r.message,
                "created_at": r.created_at,
            })
        })
        .collect();

    Ok(Json(events))
}

// ─── DB credentials by app ────────────────────────────────────────────────────
// For database-type apps (Postgres, MySQL, etc.), derives credentials from the
// app's env vars. The internal hostname is derived from app name + project namespace.

pub async fn db_credentials(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;

    let row = sqlx::query!(
        r#"SELECT a.name, a.app_type, p.name AS project_name
           FROM apps a JOIN projects p ON a.project_id = p.id
           WHERE a.id = ? AND a.project_id = ?"#,
        app_id,
        project_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("app {app_id}")))?;

    // Collect env vars for known credential keys
    let env_rows = sqlx::query!(
        r#"SELECT key_name, value FROM app_env_vars WHERE app_id = ?"#,
        app_id
    )
    .fetch_all(&state.db)
    .await?;

    let env: std::collections::HashMap<String, String> = env_rows
        .into_iter()
        .map(|r| {
            let raw = r.value.unwrap_or_default();
            let value = state.crypto.decrypt(&raw).unwrap_or(raw);
            (r.key_name, value)
        })
        .collect();

    // Internal hostname: <app-name>.<project-namespace>.svc.cluster.local
    let host = format!("{}.{}.svc.cluster.local", row.name, row.project_name);

    let (port, default_db, user_key, pass_key, db_key) = match row.app_type.as_str() {
        "POSTGRES" => (
            5432u16,
            "postgres",
            "POSTGRES_USER",
            "POSTGRES_PASSWORD",
            "POSTGRES_DB",
        ),
        "MYSQL" => (
            3306u16,
            "mysql",
            "MYSQL_USER",
            "MYSQL_ROOT_PASSWORD",
            "MYSQL_DATABASE",
        ),
        "MARIADB" => (
            3306u16,
            "mariadb",
            "MARIADB_USER",
            "MARIADB_ROOT_PASSWORD",
            "MARIADB_DATABASE",
        ),
        "MONGODB" => (
            27017u16,
            "admin",
            "MONGO_INITDB_ROOT_USERNAME",
            "MONGO_INITDB_ROOT_PASSWORD",
            "MONGO_INITDB_DATABASE",
        ),
        "REDIS" => (6379u16, "", "", "REDIS_PASSWORD", ""),
        _ => return Err(AppError::BadRequest("app is not a database type".into())),
    };

    Ok(Json(serde_json::json!({
        "app_type": row.app_type,
        "host": host,
        "port": port,
        "database": env.get(db_key).cloned().unwrap_or_else(|| default_db.to_string()),
        "username": env.get(user_key).cloned().unwrap_or_default(),
        "password": env.get(pass_key).cloned().unwrap_or_default(),
    })))
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

struct AppBasic {
    name: String,
    replicas: u8,
    status: String,
}

async fn fetch_app_basic(state: &AppState, project_id: &str, app_id: &str) -> AppResult<AppBasic> {
    let r = sqlx::query!(
        r#"SELECT name, replicas, status FROM apps WHERE id = ? AND project_id = ?"#,
        app_id,
        project_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("app {app_id}")))?;
    Ok(AppBasic {
        name: r.name,
        replicas: r.replicas,
        status: r.status,
    })
}

async fn fetch_app(
    state: &AppState,
    project_id: &str,
    app_id: &str,
) -> AppResult<serde_json::Value> {
    let r = sqlx::query!(
        r#"SELECT id, name, display_name, source_type, container_image,
                  container_registry_user, git_url, git_branch, git_ref,
                  dockerfile_path, replicas, container_command, working_dir,
                  cpu_reservation_mcores, cpu_limit_mcores,
                  mem_reservation_mb, mem_limit_mb,
                  run_as_user, run_as_group, fs_group,
                  privileged, read_only_root_fs,
                  gpu_enabled, gpu_count, gpu_model,
                  mount_ldap_files, mount_etc_hosts, mount_user_home,
                  mount_app_data, mount_app_logs, timezone,
                  anti_affinity_enabled,
                  health_check_type, health_check_path, health_check_port,
                  health_check_scheme, health_check_period, health_check_timeout, health_check_failures,
                  status, paused_at, pause_reason,
                  webhook_id, pool_id, cluster_id,
                  app_type, use_network_policy, ingress_network_policy, egress_network_policy,
                  created_at, updated_at
           FROM apps WHERE id = ? AND project_id = ?"#,
        app_id,
        project_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("app {app_id}")))?;

    Ok(serde_json::json!({
        "id": r.id,
        "name": r.name,
        "display_name": r.display_name,
        "source_type": r.source_type,
        "container_image": r.container_image,
        "container_registry_user": r.container_registry_user,
        "git_url": r.git_url,
        "git_branch": r.git_branch,
        "dockerfile_path": r.dockerfile_path,
        "replicas": r.replicas,
        "container_command": r.container_command,
        "working_dir": r.working_dir,
        "cpu_reservation_mcores": r.cpu_reservation_mcores,
        "cpu_limit_mcores": r.cpu_limit_mcores,
        "mem_reservation_mb": r.mem_reservation_mb,
        "mem_limit_mb": r.mem_limit_mb,
        "run_as_user": r.run_as_user,
        "run_as_group": r.run_as_group,
        "fs_group": r.fs_group,
        "privileged": r.privileged != 0,
        "read_only_root_fs": r.read_only_root_fs != 0,
        "gpu_enabled": r.gpu_enabled != 0,
        "gpu_count": r.gpu_count,
        "gpu_model": r.gpu_model,
        "mount_ldap_files": r.mount_ldap_files != 0,
        "mount_etc_hosts": r.mount_etc_hosts != 0,
        "mount_user_home": r.mount_user_home != 0,
        "mount_app_data": r.mount_app_data != 0,
        "mount_app_logs": r.mount_app_logs != 0,
        "timezone": r.timezone,
        "anti_affinity_enabled": r.anti_affinity_enabled != 0,
        "health_check_type": r.health_check_type,
        "health_check_path": r.health_check_path,
        "health_check_port": r.health_check_port,
        "health_check_scheme": r.health_check_scheme,
        "health_check_period": r.health_check_period,
        "health_check_timeout": r.health_check_timeout,
        "health_check_failures": r.health_check_failures,
        "status": r.status,
        "paused_at": r.paused_at,
        "pause_reason": r.pause_reason,
        "webhook_id": r.webhook_id,
        "pool_id": r.pool_id,
        "cluster_id": r.cluster_id,
        "app_type": r.app_type,
        "use_network_policy": r.use_network_policy != 0,
        "ingress_network_policy": r.ingress_network_policy,
        "egress_network_policy": r.egress_network_policy,
        "created_at": r.created_at,
        "updated_at": r.updated_at,
    }))
}

async fn project_namespace(state: &AppState, project_id: &str) -> AppResult<String> {
    sqlx::query_scalar!(r#"SELECT name FROM projects WHERE id = ?"#, project_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("project {project_id}")))
}

/// Resolve the cluster for an app.
///
/// If the app already has a `cluster_id` (from a previous deploy), reuse it.
/// Otherwise pick the least-loaded active cluster in the app's pool, bind it,
/// and return the ID.
pub(crate) async fn default_active_pool_id(state: &AppState) -> AppResult<Option<String>> {
    Ok(sqlx::query_scalar::<_, String>(
        r#"SELECT c.pool_id
           FROM clusters c
           LEFT JOIN apps a ON a.cluster_id = c.id AND a.status IN ('RUNNING','DEPLOYING')
           WHERE c.is_active = 1
           GROUP BY c.id, c.pool_id
           ORDER BY COUNT(a.id) ASC, c.created_at ASC
           LIMIT 1"#
    )
    .fetch_optional(&state.db)
    .await?)
}

async fn resolve_cluster_for_app(state: &AppState, app_id: &str) -> AppResult<String> {
    let row = sqlx::query!(
        r#"SELECT pool_id, cluster_id FROM apps WHERE id = ?"#,
        app_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("app {app_id}")))?;

    // Reuse existing binding (verify cluster is still active)
    if let Some(ref cid) = row.cluster_id {
        let active = sqlx::query_scalar!(
            r#"SELECT COUNT(*) FROM clusters WHERE id = ? AND is_active = 1"#,
            cid
        )
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

        if active > 0 {
            return Ok(cid.clone());
        }
        // Cluster deactivated — fall through to pick a new one
        tracing::warn!(app_id, cluster_id = %cid, "bound cluster deactivated, re-selecting");
    }

    let pool_id = match row.pool_id {
        Some(pool_id) => pool_id,
        None => default_active_pool_id(state).await?.ok_or_else(|| {
            AppError::BadRequest(
                "no active cluster available — create or activate a cluster before deploying".into(),
            )
        })?,
    };

    // Pick the least-loaded active cluster in the pool (fewest running apps)
    let cluster_id = sqlx::query_scalar!(
        r#"SELECT c.id
           FROM clusters c
           LEFT JOIN apps a ON a.cluster_id = c.id AND a.status IN ('RUNNING','DEPLOYING')
           WHERE c.pool_id = ? AND c.is_active = 1
           GROUP BY c.id
           ORDER BY COUNT(a.id) ASC, c.created_at ASC
           LIMIT 1"#,
        pool_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest(format!("no active cluster found in pool {pool_id}")))?;

    // Persist binding
    sqlx::query(r#"UPDATE apps SET cluster_id = ?, pool_id = ? WHERE id = ?"#)
        .bind(&cluster_id)
        .bind(&pool_id)
        .bind(app_id)
    .execute(&state.db)
    .await?;

    tracing::info!(app_id, %cluster_id, "app bound to cluster");
    Ok(cluster_id)
}

/// Returns the orchestrator type for a cluster: "K3S" (default) or "DOCKER".
async fn cluster_orchestrator(state: &AppState, cluster_id: &str) -> AppResult<String> {
    let orch: Option<String> = sqlx::query_scalar(
        "SELECT orchestrator FROM clusters WHERE id = ?",
    )
    .bind(cluster_id)
    .fetch_optional(&state.db)
    .await?;
    Ok(orch.unwrap_or_else(|| "K3S".to_string()))
}

async fn app_domains(state: &AppState, app_id: &str) -> AppResult<Vec<String>> {
    Ok(sqlx::query_scalar!(
        r#"SELECT hostname FROM app_domains WHERE app_id = ?"#,
        app_id
    )
    .fetch_all(&state.db)
    .await?)
}

// ─── Deployment events ────────────────────────────────────────────────────────

pub async fn list_events(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;

    let rows = sqlx::query!(
        r#"SELECT de.id, de.event_type, de.status, de.message, de.created_at,
                  u.username AS triggered_by_username
           FROM deployment_events de
           LEFT JOIN users u ON u.id = de.triggered_by
           WHERE de.app_id = ?
           ORDER BY de.created_at DESC
           LIMIT 100"#,
        app_id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!(rows
        .iter()
        .map(|r| serde_json::json!({
            "id": r.id,
            "event_type": r.event_type,
            "status": r.status,
            "message": r.message,
            "triggered_by": r.triggered_by_username,
            "created_at": r.created_at,
        }))
        .collect::<Vec<_>>())))
}

// ─── File mounts (inline config files) ───────────────────────────────────────

pub async fn list_file_mounts(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;
    let rows = sqlx::query!(
        r#"SELECT id, filename, mount_path FROM app_file_mounts WHERE app_id = ? ORDER BY mount_path, filename"#,
        app_id
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(serde_json::json!(rows
        .iter()
        .map(|r| serde_json::json!({
            "id": r.id,
            "filename": r.filename,
            "mount_path": r.mount_path,
        }))
        .collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct SetFileMountRequest {
    pub filename: String,
    pub mount_path: String,
    pub content: String,
}

pub async fn set_file_mount(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
    Json(body): Json<SetFileMountRequest>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO app_file_mounts (id, app_id, filename, mount_path, content)
           VALUES (?, ?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE content = ?, mount_path = ?"#,
        id,
        app_id,
        body.filename,
        body.mount_path,
        body.content,
        body.content,
        body.mount_path,
    )
    .execute(&state.db)
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "id": id })),
    ))
}

pub async fn get_file_mount(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id, file_id)): Path<(String, String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;
    let r = sqlx::query!(
        r#"SELECT id, filename, mount_path, content FROM app_file_mounts
           WHERE id = ? AND app_id = ?"#,
        file_id,
        app_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("file mount {file_id}")))?;
    Ok(Json(serde_json::json!({
        "id": r.id,
        "filename": r.filename,
        "mount_path": r.mount_path,
        "content": r.content,
    })))
}

pub async fn delete_file_mount(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, _app_id, file_id)): Path<(String, String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    sqlx::query!(r#"DELETE FROM app_file_mounts WHERE id = ?"#, file_id)
        .execute(&state.db)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Extra volumes (hostPath) ─────────────────────────────────────────────────

pub async fn list_extra_volumes(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;
    let rows = sqlx::query!(
        r#"SELECT id, host_path, mount_path, read_only FROM app_extra_volumes
           WHERE app_id = ? ORDER BY mount_path"#,
        app_id
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(serde_json::json!(rows
        .iter()
        .map(|r| serde_json::json!({
            "id": r.id,
            "host_path": r.host_path,
            "mount_path": r.mount_path,
            "read_only": r.read_only != 0,
        }))
        .collect::<Vec<_>>())))
}

#[derive(Deserialize)]
pub struct AddExtraVolumeRequest {
    pub host_path: String,
    pub mount_path: String,
    pub read_only: Option<bool>,
}

pub async fn add_extra_volume(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
    Json(body): Json<AddExtraVolumeRequest>,
) -> AppResult<impl IntoResponse> {
    // Extra volumes with arbitrary host_path are admin-only.
    // Regular users should use managed volumes (system-assigned paths).
    if !auth.is_global_admin {
        return Err(AppError::Forbidden(
            "extra host-path volumes require global admin — use managed volumes instead".into(),
        ));
    }

    // Validate host_path: absolute, no traversal, not under sensitive dirs
    crate::storage_guard::validate_admin_path(&body.host_path)?;

    super::projects::check_project_access(&state, &auth, &project_id, "ADMIN").await?;
    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO app_extra_volumes (id, app_id, host_path, mount_path, read_only)
           VALUES (?, ?, ?, ?, ?)"#,
        id,
        app_id,
        body.host_path,
        body.mount_path,
        body.read_only.unwrap_or(false) as i8,
    )
    .execute(&state.db)
    .await?;
    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "id": id })),
    ))
}

pub async fn delete_extra_volume(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, _app_id, vol_id)): Path<(String, String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    sqlx::query!(r#"DELETE FROM app_extra_volumes WHERE id = ?"#, vol_id)
        .execute(&state.db)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Monitoring: aggregate app status ────────────────────────────────────────

/// GET /api/v1/monitoring/app-status
/// Returns status for all apps accessible to the current user.
/// Global admins see all apps; regular users see apps in their projects.
pub async fn monitoring_app_status(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    let rows = if auth.is_global_admin {
        sqlx::query!(
            r#"SELECT a.id, a.name, a.project_id, p.name AS project_name,
                      a.status, a.replicas
               FROM apps a JOIN projects p ON a.project_id = p.id
               ORDER BY p.name, a.name"#
        )
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "app_id": r.id,
                "app_name": r.name,
                "project_id": r.project_id,
                "project_name": r.project_name,
                "status": r.status,
                "replicas": r.replicas,
            })
        })
        .collect::<Vec<_>>()
    } else {
        sqlx::query!(
            r#"SELECT a.id, a.name, a.project_id, p.name AS project_name,
                      a.status, a.replicas
               FROM apps a
               JOIN projects p ON a.project_id = p.id
               JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = ?
               ORDER BY p.name, a.name"#,
            auth.user_id
        )
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "app_id": r.id,
                "app_name": r.name,
                "project_id": r.project_id,
                "project_name": r.project_name,
                "status": r.status,
                "replicas": r.replicas,
            })
        })
        .collect::<Vec<_>>()
    };

    Ok(Json(rows))
}

// ─── Basic auth ───────────────────────────────────────────────────────────────

/// GET /api/v1/projects/:pid/apps/:aid/basic-auth
pub async fn get_basic_auth(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;
    let row = sqlx::query!(
        r#"SELECT id, username FROM app_basic_auth WHERE app_id = ?"#,
        app_id
    )
    .fetch_optional(&state.db)
    .await?;

    Ok(Json(match row {
        Some(r) => serde_json::json!({ "id": r.id, "username": r.username }),
        None => serde_json::json!(null),
    }))
}

#[derive(Deserialize)]
pub struct BasicAuthRequest {
    pub username: String,
    pub password: String,
}

/// PUT /api/v1/projects/:pid/apps/:aid/basic-auth
pub async fn put_basic_auth(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
    Json(body): Json<BasicAuthRequest>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    let encrypted = state
        .crypto
        .encrypt(&body.password)
        .unwrap_or_else(|_| body.password.clone());
    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO app_basic_auth (id, app_id, username, password)
           VALUES (?, ?, ?, ?)
           ON DUPLICATE KEY UPDATE username = VALUES(username), password = VALUES(password)"#,
        id,
        app_id,
        body.username,
        encrypted
    )
    .execute(&state.db)
    .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// DELETE /api/v1/projects/:pid/apps/:aid/basic-auth
pub async fn delete_basic_auth(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    sqlx::query!(r#"DELETE FROM app_basic_auth WHERE app_id = ?"#, app_id)
        .execute(&state.db)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Managed volumes (system-managed hostPath) ────────────────────────────────

/// GET /api/v1/projects/:pid/apps/:aid/managed-volumes
pub async fn list_managed_volumes(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;
    let rows = sqlx::query!(
        r#"SELECT id, name, container_mount_path, host_path, share_with_others, shared_volume_id, created_at
           FROM app_managed_volumes WHERE app_id = ? ORDER BY created_at"#,
        app_id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "name": r.name,
                    "container_mount_path": r.container_mount_path,
                    "host_path": r.host_path,
                    "share_with_others": r.share_with_others != 0,
                    "shared_volume_id": r.shared_volume_id,
                    "created_at": r.created_at,
                })
            })
            .collect::<Vec<_>>(),
    ))
}

#[derive(Deserialize)]
pub struct CreateManagedVolumeRequest {
    pub name: String,
    pub container_mount_path: String,
    pub share_with_others: Option<bool>,
    pub shared_volume_id: Option<String>,
}

/// POST /api/v1/projects/:pid/apps/:aid/managed-volumes
pub async fn create_managed_volume(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
    Json(body): Json<CreateManagedVolumeRequest>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    let id = Uuid::new_v4().to_string();
    let storage_root = &state.config.storage.root_path;
    let host_path = format!("{}/projects/{}/volumes/{}", storage_root, project_id, id);

    // Defense-in-depth: verify the generated path is under storage_root
    crate::storage_guard::validate_user_path(&host_path, storage_root)?;

    sqlx::query!(
        r#"INSERT INTO app_managed_volumes
           (id, app_id, name, container_mount_path, host_path, share_with_others, shared_volume_id)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        id,
        app_id,
        body.name,
        body.container_mount_path,
        host_path,
        body.share_with_others.unwrap_or(false) as i8,
        body.shared_volume_id,
    )
    .execute(&state.db)
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({
            "id": id,
            "host_path": host_path,
        })),
    ))
}

#[derive(Deserialize)]
pub struct UpdateManagedVolumeRequest {
    pub name: Option<String>,
    pub container_mount_path: Option<String>,
    pub share_with_others: Option<bool>,
}

/// PUT /api/v1/projects/:pid/apps/:aid/managed-volumes/:vid
pub async fn update_managed_volume(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, _app_id, vol_id)): Path<(String, String, String)>,
    Json(body): Json<UpdateManagedVolumeRequest>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    if let Some(name) = &body.name {
        sqlx::query!(
            r#"UPDATE app_managed_volumes SET name = ? WHERE id = ?"#,
            name,
            vol_id
        )
        .execute(&state.db)
        .await?;
    }
    if let Some(path) = &body.container_mount_path {
        sqlx::query!(
            r#"UPDATE app_managed_volumes SET container_mount_path = ? WHERE id = ?"#,
            path,
            vol_id
        )
        .execute(&state.db)
        .await?;
    }
    if let Some(share) = body.share_with_others {
        sqlx::query!(
            r#"UPDATE app_managed_volumes SET share_with_others = ? WHERE id = ?"#,
            share as i8,
            vol_id
        )
        .execute(&state.db)
        .await?;
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// DELETE /api/v1/projects/:pid/apps/:aid/managed-volumes/:vid
pub async fn delete_managed_volume(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, _app_id, vol_id)): Path<(String, String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    sqlx::query!(r#"DELETE FROM app_managed_volumes WHERE id = ?"#, vol_id)
        .execute(&state.db)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// GET /api/v1/projects/:pid/managed-volumes/shareable?excludeAppId=
pub async fn list_shareable_volumes(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(project_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;
    let exclude_app_id = params.get("excludeAppId").map(String::as_str).unwrap_or("");

    let rows = sqlx::query!(
        r#"SELECT v.id, v.name, v.app_id, v.container_mount_path, v.host_path
           FROM app_managed_volumes v
           JOIN apps a ON a.id = v.app_id
           WHERE a.project_id = ? AND v.share_with_others = 1 AND v.app_id != ?
           ORDER BY v.created_at"#,
        project_id,
        exclude_app_id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "name": r.name,
                    "app_id": r.app_id,
                    "container_mount_path": r.container_mount_path,
                    "host_path": r.host_path,
                })
            })
            .collect::<Vec<_>>(),
    ))
}

/// GET /api/v1/projects/:pid/apps/:aid/managed-volumes/:vid/usage
pub async fn managed_volume_usage(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, _app_id, vol_id)): Path<(String, String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;
    let row = sqlx::query!(
        r#"SELECT host_path FROM app_managed_volumes WHERE id = ?"#,
        vol_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("volume {vol_id}")))?;

    // Try to get disk usage via du command
    let usage_bytes: u64 = std::process::Command::new("du")
        .args(["-sb", &row.host_path])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .and_then(|s| s.split_whitespace().next().and_then(|n| n.parse().ok()))
        .unwrap_or(0);

    Ok(Json(serde_json::json!({
        "host_path": row.host_path,
        "usage_bytes": usage_bytes,
    })))
}

// ─── Managed volume backup schedules ─────────────────────────────────────────

/// GET /api/v1/projects/:pid/apps/:aid/managed-volumes/:vid/backups
pub async fn list_volume_backups(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, _app_id, vol_id)): Path<(String, String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;
    let rows = sqlx::query!(
        r#"SELECT id, s3_target_id, cron_expr, retention_days, use_db_backup, is_active,
                  last_run_at, last_run_status, created_at
           FROM app_volume_backups WHERE volume_id = ? ORDER BY created_at"#,
        vol_id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "s3_target_id": r.s3_target_id,
                    "cron_expr": r.cron_expr,
                    "retention_days": r.retention_days,
                    "use_db_backup": r.use_db_backup != 0,
                    "is_active": r.is_active != 0,
                    "last_run_at": r.last_run_at,
                    "last_run_status": r.last_run_status,
                    "created_at": r.created_at,
                })
            })
            .collect::<Vec<_>>(),
    ))
}

#[derive(Deserialize)]
pub struct CreateVolumeBackupRequest {
    pub s3_target_id: String,
    pub cron_expr: String,
    pub retention_days: Option<u16>,
    pub use_db_backup: Option<bool>,
}

/// POST /api/v1/projects/:pid/apps/:aid/managed-volumes/:vid/backups
pub async fn create_volume_backup(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, _app_id, vol_id)): Path<(String, String, String)>,
    Json(body): Json<CreateVolumeBackupRequest>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO app_volume_backups
           (id, volume_id, s3_target_id, cron_expr, retention_days, use_db_backup)
           VALUES (?, ?, ?, ?, ?, ?)"#,
        id,
        vol_id,
        body.s3_target_id,
        body.cron_expr,
        body.retention_days.unwrap_or(7),
        body.use_db_backup.unwrap_or(false) as i8,
    )
    .execute(&state.db)
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "id": id })),
    ))
}

#[derive(Deserialize)]
pub struct UpdateVolumeBackupRequest {
    pub cron_expr: Option<String>,
    pub retention_days: Option<u16>,
    pub is_active: Option<bool>,
}

/// PUT /api/v1/projects/:pid/apps/:aid/managed-volumes/:vid/backups/:bid
pub async fn update_volume_backup(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, _app_id, _vol_id, backup_id)): Path<(String, String, String, String)>,
    Json(body): Json<UpdateVolumeBackupRequest>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    if let Some(cron) = &body.cron_expr {
        sqlx::query!(
            r#"UPDATE app_volume_backups SET cron_expr = ? WHERE id = ?"#,
            cron,
            backup_id
        )
        .execute(&state.db)
        .await?;
    }
    if let Some(days) = body.retention_days {
        sqlx::query!(
            r#"UPDATE app_volume_backups SET retention_days = ? WHERE id = ?"#,
            days,
            backup_id
        )
        .execute(&state.db)
        .await?;
    }
    if let Some(active) = body.is_active {
        sqlx::query!(
            r#"UPDATE app_volume_backups SET is_active = ? WHERE id = ?"#,
            active as i8,
            backup_id
        )
        .execute(&state.db)
        .await?;
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// DELETE /api/v1/projects/:pid/apps/:aid/managed-volumes/:vid/backups/:bid
pub async fn delete_volume_backup(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, _app_id, _vol_id, backup_id)): Path<(String, String, String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    sqlx::query!(r#"DELETE FROM app_volume_backups WHERE id = ?"#, backup_id)
        .execute(&state.db)
        .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// POST /api/v1/projects/:pid/apps/:aid/managed-volumes/:vid/backups/:bid/run
/// Trigger an immediate backup run (stub — actual execution handled by scheduler)
pub async fn run_volume_backup(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, _app_id, _vol_id, backup_id)): Path<(String, String, String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;
    // Mark as running — actual job execution TBD
    sqlx::query!(
        r#"UPDATE app_volume_backups SET last_run_at = NOW(), last_run_status = 'RUNNING' WHERE id = ?"#,
        backup_id
    )
    .execute(&state.db)
    .await?;
    Ok(Json(serde_json::json!({ "status": "triggered" })))
}

// ─── Monitoring aggregate ─────────────────────────────────────────────────────

/// GET /api/v1/monitoring/apps
/// Returns all accessible apps enriched with latest CPU/RAM metrics.
pub async fn monitoring_apps(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    use crate::metrics::names::{APP_CPU_USED_MCORES, APP_MEM_USED_BYTES};
    use crate::metrics::types::MetricSelector;

    let rows = if auth.is_global_admin {
        sqlx::query!(
            r#"SELECT a.id, a.name, a.project_id, p.name AS project_name,
                      a.status, a.replicas
               FROM apps a JOIN projects p ON a.project_id = p.id
               ORDER BY p.name, a.name"#
        )
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(|r| {
            (
                r.id,
                r.name,
                r.project_id,
                r.project_name,
                r.status,
                r.replicas,
            )
        })
        .collect::<Vec<_>>()
    } else {
        sqlx::query!(
            r#"SELECT a.id, a.name, a.project_id, p.name AS project_name,
                      a.status, a.replicas
               FROM apps a
               JOIN projects p ON a.project_id = p.id
               JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = ?
               ORDER BY p.name, a.name"#,
            auth.user_id
        )
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(|r| {
            (
                r.id,
                r.name,
                r.project_id,
                r.project_name,
                r.status,
                r.replicas,
            )
        })
        .collect::<Vec<_>>()
    };

    let mut result = Vec::new();
    for (app_id, app_name, project_id, project_name, status, replicas) in rows {
        let cpu_pts = state
            .metrics
            .query_latest(MetricSelector::new(APP_CPU_USED_MCORES).label("app_id", app_id.clone()))
            .await
            .unwrap_or_default();
        let ram_pts = state
            .metrics
            .query_latest(MetricSelector::new(APP_MEM_USED_BYTES).label("app_id", app_id.clone()))
            .await
            .unwrap_or_default();

        let cpu_mcores = cpu_pts.first().map(|p| p.value).unwrap_or(0.0);
        let ram_bytes = ram_pts.first().map(|p| p.value).unwrap_or(0.0);

        result.push(serde_json::json!({
            "app_id": app_id,
            "app_name": app_name,
            "project_id": project_id,
            "project_name": project_name,
            "status": status,
            "replicas": replicas,
            "cpu_mcores": cpu_mcores,
            "ram_bytes": ram_bytes,
        }));
    }

    Ok(Json(result))
}

/// GET /api/v1/monitoring/managed-volumes
/// Returns all accessible managed volumes with disk usage info.
pub async fn monitoring_managed_volumes(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    let rows = if auth.is_global_admin {
        sqlx::query!(
            r#"SELECT mv.id, mv.name, mv.container_mount_path, mv.host_path,
                      mv.app_id, a.name AS app_name,
                      a.project_id, p.name AS project_name
               FROM app_managed_volumes mv
               JOIN apps a ON a.id = mv.app_id
               JOIN projects p ON p.id = a.project_id
               ORDER BY p.name, a.name, mv.name"#
        )
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "name": r.name,
                "container_mount_path": r.container_mount_path,
                "host_path": r.host_path,
                "app_id": r.app_id,
                "app_name": r.app_name,
                "project_id": r.project_id,
                "project_name": r.project_name,
                "usage_bytes": null,
            })
        })
        .collect::<Vec<_>>()
    } else {
        sqlx::query!(
            r#"SELECT mv.id, mv.name, mv.container_mount_path, mv.host_path,
                      mv.app_id, a.name AS app_name,
                      a.project_id, p.name AS project_name
               FROM app_managed_volumes mv
               JOIN apps a ON a.id = mv.app_id
               JOIN projects p ON p.id = a.project_id
               JOIN project_members pm ON pm.project_id = p.id AND pm.user_id = ?
               ORDER BY p.name, a.name, mv.name"#,
            auth.user_id
        )
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "name": r.name,
                "container_mount_path": r.container_mount_path,
                "host_path": r.host_path,
                "app_id": r.app_id,
                "app_name": r.app_name,
                "project_id": r.project_id,
                "project_name": r.project_name,
                "usage_bytes": null,
            })
        })
        .collect::<Vec<_>>()
    };

    Ok(Json(rows))
}
