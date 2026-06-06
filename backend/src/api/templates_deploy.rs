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
//
// Semantics: declaring a requirement is optional, but once declared it is
// MANDATORY at deploy time. The `required` field is no longer honoured and
// `skip` is not a valid binding mode.

#[derive(Deserialize, Clone)]
struct Requirement {
    key: String,
    kind: String,    // "database" | "objstore" | "cache" | "mq" | "smtp"
    #[serde(default)]
    engine: String,  // "mysql" / "postgres" / "rabbitmq" / "s3" / …
    #[serde(default)]
    label: Option<String>,
    /// env-var injection: { logical -> ENV_KEY }
    #[serde(default)]
    env_mapping: serde_json::Map<String, serde_json::Value>,
    /// File-mount injection: each entry is rendered with minijinja using the
    /// resolved attributes (host, port, password, ...) and written to
    /// app_file_mounts.
    #[serde(default)]
    config_files: Vec<ConfigFileSpec>,
}

#[derive(Deserialize, Clone)]
struct ConfigFileSpec {
    /// Absolute path inside the container (e.g. "/etc/app/db.yml")
    path: String,
    /// minijinja template; placeholders use {{ host }}, {{ port }}, …
    template: String,
}

struct ResolvedBinding {
    /// Value persisted in app_template_bindings.binding_kind.
    /// One of: database_instance | s3_target | mq_endpoint | smtp_endpoint | redis_endpoint
    kind: String,
    ref_id: String,
    provisioned: bool,
    /// Logical attribute map used to render env_mapping AND config_files.
    /// Keys are kind-specific logical names (host, port, password, ...).
    attributes: std::collections::BTreeMap<String, String>,
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
    // Every declared requirement MUST have a binding (no skip mode).
    for req in &requirements {
        let provided = body.bindings.iter().find(|b| b.requirement_key == req.key);
        match provided {
            None => {
                return Err(AppError::BadRequest(format!(
                    "binding required for declared requirement '{}'",
                    req.key
                )))
            }
            Some(b) if b.mode == "skip" => {
                return Err(AppError::BadRequest(format!(
                    "declared requirement '{}' cannot be skipped",
                    req.key
                )))
            }
            _ => {}
        }
    }

    // ── Pre-check binding quotas (P2c) ─────────────────────────────────────
    // For each incoming binding, compute the "extra distinct refs" this deploy
    // would add to the project's usage, then have managed_usage validate it
    // against per-kind project quotas.
    {
        use std::collections::HashMap;
        let mut delta: HashMap<String, i64> = HashMap::new();
        for (req, binding) in requirements.iter().filter_map(|r| {
            body.bindings.iter().find(|b| b.requirement_key == r.key).map(|b| (r, b))
        }) {
            let kind = match req.kind.as_str() {
                "database" => "database_instance",
                "cache"    => "redis_endpoint",
                "objstore" => "s3_target",
                "mq"       => "mq_endpoint",
                "smtp"     => "smtp_endpoint",
                _ => continue,
            };
            // mode=provision is always new. mode=managed only counts if the
            // project isn't already bound to that ref via another app.
            let counts_as_new = if binding.mode == "provision" {
                true
            } else if let Some(ref_id) = binding.managed_ref_id.as_deref() {
                let existing: i64 = sqlx::query_scalar(
                    "SELECT COUNT(*) FROM app_template_bindings b \
                     JOIN apps a ON a.id = b.app_id \
                     WHERE a.project_id = ? AND b.binding_kind = ? AND b.binding_ref_id = ?",
                )
                .bind(&project_id)
                .bind(kind)
                .bind(ref_id)
                .fetch_one(&state.db)
                .await?;
                existing == 0
            } else {
                false
            };
            if counts_as_new {
                *delta.entry(kind.to_string()).or_insert(0) += 1;
            }
        }
        super::managed_usage::check_binding_allowed(&state, &project_id, &delta).await?;
    }

    // ── Resolve bindings (one per provided binding) ────────────────────────
    let mut env_vars_from_bindings: Vec<(String, String, bool)> = Vec::new();
    let mut file_mounts_from_bindings: Vec<(String, String)> = Vec::new(); // (path, rendered_content)
    let mut binding_records: Vec<ResolvedBinding> = Vec::new();
    let mut binding_requirement_keys: Vec<String> = Vec::new();

    for req in &requirements {
        let Some(binding) = body.bindings.iter().find(|b| b.requirement_key == req.key) else {
            continue;
        };
        let resolved = resolve_binding(&state, &project_id, &auth, req, binding).await?;

        // Env-var injection
        for (logical, target) in &req.env_mapping {
            let Some(target_key) = target.as_str() else { continue };
            if let Some(value) = resolved.attributes.get(logical) {
                env_vars_from_bindings.push((target_key.to_string(), value.clone(), resolved.is_secret));
            }
        }

        // Config-file injection: render each spec with minijinja
        for cf in &req.config_files {
            let rendered = render_config_file(&cf.template, &resolved.attributes)
                .map_err(|e| AppError::BadRequest(format!(
                    "config file render failed for requirement '{}': {e}", req.key
                )))?;
            file_mounts_from_bindings.push((cf.path.clone(), rendered));
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
             container_image, image_registry_id, replicas, \
             webhook_id, app_type, use_network_policy, \
             ingress_network_policy, egress_network_policy, \
             health_check_type, health_check_scheme, health_check_period, \
             health_check_timeout, health_check_failures, \
             dockerfile_path, privileged, read_only_root_fs, \
             gpu_enabled, anti_affinity_enabled, \
             mount_ldap_files, mount_etc_hosts, mount_user_home, \
             mount_app_data, mount_app_logs) \
         VALUES (?, ?, ?, ?, ?, ?, ?, \
                 ?, ?, ?, \
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
    // P0.3 — propagate template's registry FK so the k8s deployer can
    // synthesize an imagePullSecret from image_registries.{username,password}.
    .bind(&tpl.image_registry_id)
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

    // ── Config-file injection (rendered via minijinja from binding attrs) ──
    for (path, content) in file_mounts_from_bindings {
        let (mount_path, filename) = split_path(&path);
        sqlx::query(
            "INSERT INTO app_file_mounts (id, app_id, mount_path, filename, content) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&app_id)
        .bind(&mount_path)
        .bind(&filename)
        .bind(&content)
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
    let managed_id = || {
        binding.managed_ref_id.as_deref()
            .ok_or_else(|| AppError::BadRequest("managed_ref_id required".into()))
    };
    match (req.kind.as_str(), binding.mode.as_str()) {
        // ── Database (MySQL/Postgres/MongoDB) — managed or provision ──
        ("database", "managed") => {
            resolve_existing_database(state, project_id, managed_id()?).await
        }
        ("database", "provision") => {
            let cluster_id = binding.provision_cluster_id.as_deref()
                .ok_or_else(|| AppError::BadRequest("provision_cluster_id required".into()))?;
            let hint = binding.provision_name_hint.as_deref().unwrap_or(&req.key);
            provision_new_database(state, project_id, auth, cluster_id, hint).await
        }
        // ── Cache (Redis) — managed only; provision unsupported in P2 ──
        ("cache", "managed") => resolve_existing_redis(state, managed_id()?).await,
        ("cache", "provision") => Err(AppError::BadRequest(
            "Redis provisioning is not supported — register a redis_endpoint and use mode=managed".into(),
        )),
        // ── Object storage (S3) — managed only ──
        ("objstore", "managed") => resolve_existing_s3(state, managed_id()?).await,
        ("objstore", "provision") => Err(AppError::BadRequest(
            "S3 provisioning is not supported — pick an existing target".into(),
        )),
        // ── Message queue ──
        ("mq", "managed") => resolve_existing_mq(state, managed_id()?).await,
        ("mq", "provision") => Err(AppError::BadRequest(
            "MQ provisioning is not supported — register an mq_endpoint and use mode=managed".into(),
        )),
        // ── SMTP relay ──
        ("smtp", "managed") => resolve_existing_smtp(state, managed_id()?).await,
        ("smtp", "provision") => Err(AppError::BadRequest(
            "SMTP provisioning is not supported — register an smtp_endpoint and use mode=managed".into(),
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

    Ok(ResolvedBinding {
        kind: "database_instance".into(),
        ref_id: instance_id.to_string(),
        provisioned: false,
        attributes: db_attrs(&row.host, row.port as u16, &row.db_name, &row.db_user, &password),
        is_secret: true,
    })
}

async fn provision_new_database(
    state: &AppState,
    project_id: &str,
    auth: &AuthUser,
    cluster_id: &str,
    name_hint: &str,
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

    Ok(ResolvedBinding {
        kind: "database_instance".into(),
        ref_id: instance_id,
        provisioned: true,
        attributes: db_attrs(
            &cluster.host, cluster.port as u16,
            &db_name, &db_user, &db_password,
        ),
        is_secret: true,
    })
}

async fn resolve_existing_s3(
    state: &AppState,
    target_id: &str,
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
    let mut attrs = std::collections::BTreeMap::new();
    attrs.insert("endpoint".into(), row.endpoint);
    attrs.insert("region".into(), row.region.unwrap_or_default());
    attrs.insert("bucket".into(), row.bucket_name);
    attrs.insert("access_key".into(), row.access_key_id);
    attrs.insert("secret_key".into(), secret);

    Ok(ResolvedBinding {
        kind: "s3_target".into(),
        ref_id: target_id.to_string(),
        provisioned: false,
        attributes: attrs,
        is_secret: true,
    })
}

async fn resolve_existing_mq(state: &AppState, id: &str) -> AppResult<ResolvedBinding> {
    #[derive(sqlx::FromRow)]
    struct Row {
        host: String, port: u16, vhost: String,
        username: String, password: String, tls_enabled: i8,
    }
    let row: Row = sqlx::query_as(
        "SELECT host, port, vhost, username, password, tls_enabled \
         FROM mq_endpoints WHERE id = ? AND is_active = 1",
    )
    .bind(id).fetch_optional(&state.db).await?
    .ok_or_else(|| AppError::NotFound(format!("mq_endpoint {id}")))?;

    let password = state.crypto.decrypt(&row.password)?;
    let scheme = if row.tls_enabled != 0 { "amqps" } else { "amqp" };
    let mut attrs = std::collections::BTreeMap::new();
    attrs.insert("host".into(), row.host.clone());
    attrs.insert("port".into(), row.port.to_string());
    attrs.insert("vhost".into(), row.vhost.clone());
    attrs.insert("user".into(), row.username.clone());
    attrs.insert("username".into(), row.username.clone());
    attrs.insert("password".into(), password.clone());
    attrs.insert("tls".into(), if row.tls_enabled != 0 { "true".into() } else { "false".into() });
    // Convenient pre-built URL
    let vh = if row.vhost == "/" { "".to_string() } else { format!("/{}", row.vhost.trim_start_matches('/')) };
    attrs.insert(
        "url".into(),
        format!("{scheme}://{}:{}@{}:{}{}", row.username, password, row.host, row.port, vh),
    );

    Ok(ResolvedBinding {
        kind: "mq_endpoint".into(),
        ref_id: id.to_string(),
        provisioned: false,
        attributes: attrs,
        is_secret: true,
    })
}

async fn resolve_existing_smtp(state: &AppState, id: &str) -> AppResult<ResolvedBinding> {
    #[derive(sqlx::FromRow)]
    struct Row {
        host: String, port: u16,
        username: Option<String>, password: Option<String>,
        from_address: Option<String>, tls_enabled: i8,
    }
    let row: Row = sqlx::query_as(
        "SELECT host, port, username, password, from_address, tls_enabled \
         FROM smtp_endpoints WHERE id = ? AND is_active = 1",
    )
    .bind(id).fetch_optional(&state.db).await?
    .ok_or_else(|| AppError::NotFound(format!("smtp_endpoint {id}")))?;

    let password = match row.password.as_deref() {
        Some(enc) if !enc.is_empty() => state.crypto.decrypt(enc)?,
        _ => String::new(),
    };
    let mut attrs = std::collections::BTreeMap::new();
    attrs.insert("host".into(), row.host);
    attrs.insert("port".into(), row.port.to_string());
    attrs.insert("user".into(), row.username.clone().unwrap_or_default());
    attrs.insert("username".into(), row.username.unwrap_or_default());
    attrs.insert("password".into(), password);
    attrs.insert("from_address".into(), row.from_address.clone().unwrap_or_default());
    attrs.insert("from".into(), row.from_address.unwrap_or_default());
    attrs.insert("tls".into(), if row.tls_enabled != 0 { "true".into() } else { "false".into() });

    Ok(ResolvedBinding {
        kind: "smtp_endpoint".into(),
        ref_id: id.to_string(),
        provisioned: false,
        attributes: attrs,
        is_secret: true,
    })
}

async fn resolve_existing_redis(state: &AppState, id: &str) -> AppResult<ResolvedBinding> {
    #[derive(sqlx::FromRow)]
    struct Row {
        host: String, port: u16, password: Option<String>,
        db_index: i16, tls_enabled: i8,
    }
    let row: Row = sqlx::query_as(
        "SELECT host, port, password, db_index, tls_enabled \
         FROM redis_endpoints WHERE id = ? AND is_active = 1",
    )
    .bind(id).fetch_optional(&state.db).await?
    .ok_or_else(|| AppError::NotFound(format!("redis_endpoint {id}")))?;

    let password = match row.password.as_deref() {
        Some(enc) if !enc.is_empty() => state.crypto.decrypt(enc)?,
        _ => String::new(),
    };
    let scheme = if row.tls_enabled != 0 { "rediss" } else { "redis" };
    let auth_part = if password.is_empty() {
        String::new()
    } else {
        format!(":{password}@")
    };

    let mut attrs = std::collections::BTreeMap::new();
    attrs.insert("host".into(), row.host.clone());
    attrs.insert("port".into(), row.port.to_string());
    attrs.insert("password".into(), password.clone());
    attrs.insert("db".into(), row.db_index.to_string());
    attrs.insert("tls".into(), if row.tls_enabled != 0 { "true".into() } else { "false".into() });
    attrs.insert(
        "url".into(),
        format!("{scheme}://{auth_part}{}:{}/{}", row.host, row.port, row.db_index),
    );

    Ok(ResolvedBinding {
        kind: "redis_endpoint".into(),
        ref_id: id.to_string(),
        provisioned: false,
        attributes: attrs,
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

/// Standard logical-attribute map for a database binding.
/// Both `name`/`database` and `user`/`username` are populated so env_mapping
/// and config templates can use whichever name is idiomatic.
fn db_attrs(
    host: &str, port: u16, db_name: &str, db_user: &str, db_password: &str,
) -> std::collections::BTreeMap<String, String> {
    let mut m = std::collections::BTreeMap::new();
    m.insert("host".into(), host.to_string());
    m.insert("port".into(), port.to_string());
    m.insert("name".into(), db_name.to_string());
    m.insert("database".into(), db_name.to_string());
    m.insert("user".into(), db_user.to_string());
    m.insert("username".into(), db_user.to_string());
    m.insert("password".into(), db_password.to_string());
    m
}

/// Render a config-file template with the given attribute map using minijinja.
/// Placeholders use the standard `{{ var }}` syntax; if/for/filters all work.
fn render_config_file(
    template: &str,
    attrs: &std::collections::BTreeMap<String, String>,
) -> Result<String, String> {
    let env = minijinja::Environment::new();
    let tmpl = env.template_from_str(template).map_err(|e| e.to_string())?;
    let ctx: std::collections::BTreeMap<String, minijinja::Value> = attrs
        .iter()
        .map(|(k, v)| (k.clone(), minijinja::Value::from(v.clone())))
        .collect();
    tmpl.render(ctx).map_err(|e| e.to_string())
}

/// Split "/etc/app/db.yml" -> ("/etc/app", "db.yml") for app_file_mounts.
fn split_path(full_path: &str) -> (String, String) {
    match full_path.rsplit_once('/') {
        Some(("", filename)) => ("/".to_string(), filename.to_string()),
        Some((dir, filename)) => (dir.to_string(), filename.to_string()),
        None => (".".to_string(), full_path.to_string()),
    }
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
