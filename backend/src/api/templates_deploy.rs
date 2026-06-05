//! Deploy an app from a template, resolving its service requirements to
//! managed database / object-storage instances and injecting connection
//! credentials as env vars on the new app.
//!
//! Endpoint:
//!   POST /api/v1/projects/:project_id/apps/from-template
//!
//! Phase 2a scope:
//!   - Resolves `requirements[]` from the template into env-var bindings
//!   - Two binding modes:
//!       * "managed"   = bind to an existing database_instance or s3_target
//!       * "provision" = create a new database_instance on a chosen cluster
//!         (S3 provisioning is out of scope for P2a — only "managed")
//!       * "skip"      = only allowed when requirement.required == false
//!   - Writes app_template_bindings rows so deletes can cascade-clean
//!   - Sub-resource (ports / volumes / file mounts / env vars / domains)
//!     materialization from the template spec
//!
//! Cleanup of provisioned managed resources lives in
//! `cleanup_bindings_for_app()`, called from `apps::delete_app`.

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

// ── Request shape ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct DeployRequest {
    pub template_id: String,
    pub app_name: String,
    /// Optional human-friendly display name. Defaults to `app_name`.
    #[serde(default)]
    pub display_name: Option<String>,
    /// One per non-skipped template requirement.
    #[serde(default)]
    pub bindings: Vec<BindingRequest>,
    /// Overrides for template inputs (input.key -> new value).
    /// Special key `containerImageSource` overrides the rendered image_ref.
    #[serde(default)]
    pub input_overrides: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize)]
pub struct BindingRequest {
    pub requirement_key: String,
    /// "managed" | "provision" | "skip"
    pub mode: String,
    /// For mode=managed: which existing database_instance or s3_target to use.
    #[serde(default)]
    pub managed_ref_id: Option<String>,
    /// For mode=provision: which database_cluster to provision on, and the
    /// per-database identifier the user wants (becomes part of db_name).
    #[serde(default)]
    pub provision_cluster_id: Option<String>,
    #[serde(default)]
    pub provision_name_hint: Option<String>,
}

// ── Template requirement shape (mirrors the JSON stored in app_templates.requirements) ──

#[derive(Deserialize, Clone)]
struct Requirement {
    key: String,
    kind: String,    // "database" | "objstore" | "cache"
    #[serde(default)]
    engine: String,  // "mysql" / "postgres" / "mariadb" / "redis" / "s3"
    #[serde(default)]
    required: bool,
    #[serde(default)]
    env_mapping: serde_json::Map<String, serde_json::Value>,
}

struct ResolvedBinding {
    kind: String,        // 'database_instance' | 's3_target' (for the bindings table)
    ref_id: String,
    provisioned: bool,
    env_vars: Vec<(String, String)>, // (key_name, value) to insert into app_env_vars
    is_secret: bool,
}

// ── Handler ─────────────────────────────────────────────────────────────────

pub async fn deploy_from_template(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(project_id): Path<String>,
    Json(body): Json<DeployRequest>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;

    if body.app_name.is_empty() {
        return Err(AppError::BadRequest("app_name is required".into()));
    }

    // ── Load the template ──────────────────────────────────────────────────
    #[derive(sqlx::FromRow)]
    struct TplRow {
        spec: serde_json::Value,
        requirements: serde_json::Value,
        inputs: serde_json::Value,
        image_registry_id: Option<String>,
        image_repository: String,
        image_tag: String,
        image_digest: Option<String>,
    }
    let tpl: TplRow = sqlx::query_as(
        "SELECT spec, requirements, inputs, image_registry_id, image_repository, \
                image_tag, image_digest \
         FROM app_templates WHERE id = ? AND is_active = 1",
    )
    .bind(&body.template_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("template {}", body.template_id)))?;

    let requirements: Vec<Requirement> =
        serde_json::from_value(tpl.requirements.clone()).map_err(|e| {
            AppError::BadRequest(format!("template requirements invalid: {e}"))
        })?;

    // ── Validate bindings ──────────────────────────────────────────────────
    for req in &requirements {
        let provided = body.bindings.iter().find(|b| b.requirement_key == req.key);
        match (req.required, provided) {
            (true, None) => {
                return Err(AppError::BadRequest(format!(
                    "binding required for '{}'",
                    req.key
                )))
            }
            (true, Some(b)) if b.mode == "skip" => {
                return Err(AppError::BadRequest(format!(
                    "requirement '{}' is required and cannot be skipped",
                    req.key
                )))
            }
            _ => {}
        }
    }

    // ── Resolve bindings (one per provided binding) ────────────────────────
    let mut env_vars_from_bindings: Vec<(String, String, bool)> = Vec::new();
    let mut binding_records: Vec<ResolvedBinding> = Vec::new();
    let mut binding_requirement_keys: Vec<String> = Vec::new();

    for req in &requirements {
        let Some(binding) = body.bindings.iter().find(|b| b.requirement_key == req.key) else {
            continue;
        };
        if binding.mode == "skip" {
            continue;
        }
        let resolved = resolve_binding(&state, &project_id, &auth, req, binding).await?;
        for (k, v) in &resolved.env_vars {
            env_vars_from_bindings.push((k.clone(), v.clone(), resolved.is_secret));
        }
        binding_requirement_keys.push(req.key.clone());
        binding_records.push(resolved);
    }

    // ── Resolve container image ────────────────────────────────────────────
    let image_override = body
        .input_overrides
        .get("containerImageSource")
        .and_then(|v| v.as_str())
        .map(String::from);
    let image_ref = if let Some(v) = image_override {
        v
    } else {
        super::templates::render_image_ref(
            &state,
            tpl.image_registry_id.as_deref(),
            &tpl.image_repository,
            &tpl.image_tag,
            tpl.image_digest.as_deref(),
        )
        .await
    };

    // ── Create the app row ─────────────────────────────────────────────────
    let spec = &tpl.spec;
    let app_model = spec
        .get("appModel")
        .ok_or_else(|| AppError::BadRequest("template spec missing appModel".into()))?;

    let app_id = Uuid::new_v4().to_string();
    let webhook_id = Uuid::new_v4().to_string();
    let pool_id = super::apps::default_active_pool_id(&state).await?;
    let display_name = body.display_name.unwrap_or_else(|| body.app_name.clone());

    let app_type = j_str(app_model, "appType", "APP");
    let source_type = j_str(app_model, "sourceType", "CONTAINER");
    let ingress_pol = j_str(app_model, "ingressNetworkPolicy", "ALLOW_ALL");
    let egress_pol = j_str(app_model, "egressNetworkPolicy", "ALLOW_ALL");
    let use_np = j_bool(app_model, "useNetworkPolicy", true) as i8;
    let replicas = j_i64(app_model, "replicas", 1) as i32;
    let hc_period = j_i64(app_model, "healthCheckPeriodSeconds", 15) as i32;
    let hc_timeout = j_i64(app_model, "healthCheckTimeoutSeconds", 10) as i32;
    let hc_failures = j_i64(app_model, "healthCheckFailureThreshold", 3) as i32;

    sqlx::query(
        "INSERT INTO apps \
            (id, project_id, pool_id, name, display_name, owner_id, source_type, \
             container_image, replicas, \
             webhook_id, app_type, use_network_policy, \
             ingress_network_policy, egress_network_policy, \
             health_check_type, health_check_scheme, health_check_period, \
             health_check_timeout, health_check_failures, \
             dockerfile_path, privileged, read_only_root_fs, \
             gpu_enabled, anti_affinity_enabled, \
             mount_ldap_files, mount_etc_hosts, mount_user_home, \
             mount_app_data, mount_app_logs) \
         VALUES (?, ?, ?, ?, ?, ?, ?, \
                 ?, ?, \
                 ?, ?, ?, \
                 ?, ?, \
                 'NONE', 'HTTP', ?, ?, ?, \
                 'Dockerfile', 0, 0, \
                 0, 0, \
                 0, 0, 0, 0, 0)",
    )
    .bind(&app_id)
    .bind(&project_id)
    .bind(&pool_id)
    .bind(&body.app_name)
    .bind(&display_name)
    .bind(&auth.user_id)
    .bind(&source_type)
    .bind(&image_ref)
    .bind(replicas)
    .bind(&webhook_id)
    .bind(&app_type)
    .bind(use_np)
    .bind(&ingress_pol)
    .bind(&egress_pol)
    .bind(hc_period)
    .bind(hc_timeout)
    .bind(hc_failures)
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref de) if de.is_unique_violation() => {
            AppError::Conflict(format!("app '{}' already exists", body.app_name))
        }
        other => AppError::Database(other),
    })?;

    // ── Sub-resources: ports / volumes / file mounts ───────────────────────
    if let Some(ports) = spec.get("appPorts").and_then(|v| v.as_array()) {
        for p in ports {
            let port = p.get("port").and_then(|v| v.as_i64()).unwrap_or(0) as u16;
            if port == 0 {
                continue;
            }
            sqlx::query(
                "INSERT INTO app_ports (id, app_id, container_port, protocol) \
                 VALUES (?, ?, ?, 'TCP')",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&app_id)
            .bind(port)
            .execute(&state.db)
            .await?;
        }
    }

    // ── Env vars: parse template's envVars block + apply input overrides + binding vars ──
    let mut env_pairs: Vec<(String, String, bool)> = Vec::new();

    // From template spec.appModel.envVars (multi-line "KEY=VALUE\n")
    if let Some(env_str) = app_model.get("envVars").and_then(|v| v.as_str()) {
        env_pairs.extend(parse_env_block(env_str).into_iter().map(|(k, v)| (k, v, false)));
    }

    // From inputs[] where isEnvVar=true (with input_overrides applied)
    if let Some(inputs) = tpl.inputs.as_array() {
        for input in inputs {
            let is_env = input.get("isEnvVar").and_then(|v| v.as_bool()).unwrap_or(false);
            if !is_env {
                continue;
            }
            let key = input.get("key").and_then(|v| v.as_str()).unwrap_or("");
            if key.is_empty() {
                continue;
            }
            let default_val = input.get("value").and_then(|v| v.as_str()).unwrap_or("");
            let val = body
                .input_overrides
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or(default_val);
            env_pairs.push((key.to_string(), val.to_string(), false));
        }
    }

    // From bindings — these may contain secrets, hence the flag
    env_pairs.extend(env_vars_from_bindings);

    for (k, v, is_secret) in env_pairs {
        sqlx::query(
            "INSERT INTO app_env_vars (id, app_id, key_name, value, is_secret) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&app_id)
        .bind(&k)
        .bind(&v)
        .bind(if is_secret { 1i8 } else { 0i8 })
        .execute(&state.db)
        .await?;
    }

    // ── Record bindings so delete can cascade-clean managed resources ──────
    for (rkey, rec) in binding_requirement_keys.iter().zip(binding_records.iter()) {
        sqlx::query(
            "INSERT INTO app_template_bindings \
                (id, app_id, requirement_key, binding_kind, binding_ref_id, provisioned) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&app_id)
        .bind(rkey)
        .bind(&rec.kind)
        .bind(&rec.ref_id)
        .bind(if rec.provisioned { 1i8 } else { 0i8 })
        .execute(&state.db)
        .await?;
    }

    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "id": app_id })),
    ))
}

// ── Binding resolution ──────────────────────────────────────────────────────

async fn resolve_binding(
    state: &AppState,
    project_id: &str,
    auth: &AuthUser,
    req: &Requirement,
    binding: &BindingRequest,
) -> AppResult<ResolvedBinding> {
    match (req.kind.as_str(), binding.mode.as_str()) {
        ("database" | "cache", "managed") => {
            let ref_id = binding
                .managed_ref_id
                .as_deref()
                .ok_or_else(|| AppError::BadRequest("managed_ref_id required".into()))?;
            resolve_existing_database(state, project_id, ref_id, &req.env_mapping).await
        }
        ("database" | "cache", "provision") => {
            let cluster_id = binding
                .provision_cluster_id
                .as_deref()
                .ok_or_else(|| AppError::BadRequest("provision_cluster_id required".into()))?;
            let hint = binding.provision_name_hint.as_deref().unwrap_or(&req.key);
            provision_new_database(state, project_id, auth, cluster_id, hint, &req.env_mapping).await
        }
        ("objstore", "managed") => {
            let ref_id = binding
                .managed_ref_id
                .as_deref()
                .ok_or_else(|| AppError::BadRequest("managed_ref_id required".into()))?;
            resolve_existing_s3(state, ref_id, &req.env_mapping).await
        }
        ("objstore", "provision") => Err(AppError::BadRequest(
            "S3 provisioning is not supported in P2a — use mode=managed and pick an existing target".into(),
        )),
        (k, m) => Err(AppError::BadRequest(format!(
            "unsupported binding: kind={k} mode={m}"
        ))),
    }
}

async fn resolve_existing_database(
    state: &AppState,
    project_id: &str,
    instance_id: &str,
    env_mapping: &serde_json::Map<String, serde_json::Value>,
) -> AppResult<ResolvedBinding> {
    #[derive(sqlx::FromRow)]
    struct Row {
        db_name: String,
        db_user: String,
        db_password: String,
        host: String,
        port: i16,
    }
    let row: Row = sqlx::query_as(
        "SELECT di.db_name, di.db_user, di.db_password, dc.host, dc.port \
         FROM database_instances di \
         JOIN database_clusters dc ON dc.id = di.cluster_id \
         WHERE di.id = ? AND di.project_id = ?",
    )
    .bind(instance_id)
    .bind(project_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("database_instance {instance_id}")))?;

    let password = state.crypto.decrypt(&row.db_password)?;
    let env_vars = map_db_env_vars(
        env_mapping,
        &row.host,
        row.port as u16,
        &row.db_name,
        &row.db_user,
        &password,
    );

    Ok(ResolvedBinding {
        kind: "database_instance".into(),
        ref_id: instance_id.to_string(),
        provisioned: false,
        env_vars,
        is_secret: true,
    })
}

async fn provision_new_database(
    state: &AppState,
    project_id: &str,
    auth: &AuthUser,
    cluster_id: &str,
    name_hint: &str,
    env_mapping: &serde_json::Map<String, serde_json::Value>,
) -> AppResult<ResolvedBinding> {
    // Sanitize name_hint to MySQL-safe identifier chars
    let clean: String = name_hint
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if clean.is_empty() {
        return Err(AppError::BadRequest("provision_name_hint produced empty identifier".into()));
    }

    let project_name = sqlx::query_scalar::<_, String>(
        "SELECT name FROM projects WHERE id = ?",
    )
    .bind(project_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("project {project_id}")))?;

    #[derive(sqlx::FromRow)]
    struct ClusterRow {
        cluster_type: String,
        host: String,
        port: i16,
        admin_user: String,
        admin_password: String,
    }
    let cluster: ClusterRow = sqlx::query_as(
        "SELECT cluster_type, host, port, admin_user, admin_password \
         FROM database_clusters WHERE id = ? AND is_active = 1",
    )
    .bind(cluster_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("database cluster {cluster_id}")))?;

    let db_name = format!("p_{}_{}", project_name, clean);
    let db_user = format!(
        "u_{}_{}",
        auth.username
            .replace(|c: char| !c.is_ascii_alphanumeric() && c != '_', "_"),
        &Uuid::new_v4().to_string()[..6]
    );
    let db_password = super::databases::generate_password();
    let admin_pass = state.crypto.decrypt(&cluster.admin_password)?;

    crate::k8s::database::provision_database(
        &cluster.cluster_type,
        &cluster.host,
        cluster.port as u16,
        &cluster.admin_user,
        &admin_pass,
        &db_name,
        &db_user,
        &db_password,
    )
    .await?;

    let encrypted_pass = state.crypto.encrypt(&db_password)?;
    let instance_id = Uuid::new_v4().to_string();
    let secret_name = format!("db-{}", &instance_id[..8]);

    sqlx::query(
        "INSERT INTO database_instances \
            (id, cluster_id, project_id, created_by, db_name, db_user, db_password, k8s_secret_name) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&instance_id)
    .bind(cluster_id)
    .bind(project_id)
    .bind(&auth.user_id)
    .bind(&db_name)
    .bind(&db_user)
    .bind(&encrypted_pass)
    .bind(&secret_name)
    .execute(&state.db)
    .await?;

    let env_vars = map_db_env_vars(
        env_mapping,
        &cluster.host,
        cluster.port as u16,
        &db_name,
        &db_user,
        &db_password,
    );

    Ok(ResolvedBinding {
        kind: "database_instance".into(),
        ref_id: instance_id,
        provisioned: true,
        env_vars,
        is_secret: true,
    })
}

async fn resolve_existing_s3(
    state: &AppState,
    target_id: &str,
    env_mapping: &serde_json::Map<String, serde_json::Value>,
) -> AppResult<ResolvedBinding> {
    #[derive(sqlx::FromRow)]
    struct Row {
        endpoint: String,
        region: Option<String>,
        access_key_id: String,
        secret_key: String,
        bucket_name: String,
    }
    let row: Row = sqlx::query_as(
        "SELECT endpoint, region, access_key_id, secret_key, bucket_name \
         FROM s3_targets WHERE id = ? AND is_active = 1",
    )
    .bind(target_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("s3_target {target_id}")))?;

    let secret = state.crypto.decrypt(&row.secret_key)?;
    let mut env_vars = Vec::new();
    for (logical, target) in env_mapping {
        let Some(target_key) = target.as_str() else { continue };
        let value = match logical.as_str() {
            "endpoint" => row.endpoint.clone(),
            "region" => row.region.clone().unwrap_or_default(),
            "bucket" => row.bucket_name.clone(),
            "access_key" => row.access_key_id.clone(),
            "secret_key" => secret.clone(),
            _ => continue,
        };
        env_vars.push((target_key.to_string(), value));
    }

    Ok(ResolvedBinding {
        kind: "s3_target".into(),
        ref_id: target_id.to_string(),
        provisioned: false,
        env_vars,
        is_secret: true,
    })
}

// ── Cleanup hook: called from apps::delete_app before the apps row is removed ──

pub async fn cleanup_bindings_for_app(state: &AppState, app_id: &str) -> AppResult<()> {
    #[derive(sqlx::FromRow)]
    struct B {
        binding_kind: String,
        binding_ref_id: String,
        provisioned: i8,
    }
    let bindings: Vec<B> = sqlx::query_as(
        "SELECT binding_kind, binding_ref_id, provisioned \
         FROM app_template_bindings WHERE app_id = ?",
    )
    .bind(app_id)
    .fetch_all(&state.db)
    .await?;

    for b in bindings {
        if b.provisioned == 0 {
            continue;
        }
        if b.binding_kind == "database_instance" {
            // Best-effort drop: errors are logged but don't block app delete.
            if let Err(e) = drop_provisioned_database(state, &b.binding_ref_id).await {
                tracing::warn!(
                    app_id = %app_id,
                    instance = %b.binding_ref_id,
                    "failed to drop provisioned database: {e}"
                );
            }
        }
        // S3 provision unsupported in P2a — nothing to clean.
    }
    Ok(())
}

async fn drop_provisioned_database(state: &AppState, instance_id: &str) -> AppResult<()> {
    #[derive(sqlx::FromRow)]
    struct Row {
        db_name: String,
        db_user: String,
        cluster_type: String,
        host: String,
        port: i16,
        admin_user: String,
        admin_password: String,
    }
    let row: Option<Row> = sqlx::query_as(
        "SELECT di.db_name, di.db_user, dc.cluster_type, dc.host, dc.port, \
                dc.admin_user, dc.admin_password \
         FROM database_instances di \
         JOIN database_clusters dc ON dc.id = di.cluster_id \
         WHERE di.id = ?",
    )
    .bind(instance_id)
    .fetch_optional(&state.db)
    .await?;
    let Some(row) = row else { return Ok(()); };

    let admin_pass = state.crypto.decrypt(&row.admin_password)?;
    crate::k8s::database::drop_database(
        &row.cluster_type,
        &row.host,
        row.port as u16,
        &row.admin_user,
        &admin_pass,
        &row.db_name,
        &row.db_user,
    )
    .await?;

    sqlx::query("DELETE FROM database_instances WHERE id = ?")
        .bind(instance_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn map_db_env_vars(
    env_mapping: &serde_json::Map<String, serde_json::Value>,
    host: &str,
    port: u16,
    db_name: &str,
    db_user: &str,
    db_password: &str,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (logical, target) in env_mapping {
        let Some(target_key) = target.as_str() else { continue };
        let value = match logical.as_str() {
            "host" => host.to_string(),
            "port" => port.to_string(),
            "name" | "database" => db_name.to_string(),
            "user" | "username" => db_user.to_string(),
            "password" => db_password.to_string(),
            _ => continue,
        };
        out.push((target_key.to_string(), value));
    }
    out
}

fn parse_env_block(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in s.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            out.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    out
}

fn j_str(v: &serde_json::Value, key: &str, default: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or(default)
        .to_string()
}
fn j_bool(v: &serde_json::Value, key: &str, default: bool) -> bool {
    v.get(key).and_then(|x| x.as_bool()).unwrap_or(default)
}
fn j_i64(v: &serde_json::Value, key: &str, default: i64) -> i64 {
    v.get(key).and_then(|x| x.as_i64()).unwrap_or(default)
}
