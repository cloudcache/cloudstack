//! Build jobs: trigger CI/CD builds for GIT-source apps, track status and logs.
//!
//! Only apps with source_type = 'GIT' can be built.
//! Build runs as a K8s Job; logs are streamed from the Job pod via SSE.

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

// ─── List builds ─────────────────────────────────────────────────────────────

/// GET /projects/:pid/apps/:aid/builds
pub async fn list_builds(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;

    let rows = sqlx::query!(
        r#"SELECT bj.id, bj.k8s_job_name, bj.git_commit_hash, bj.image_tag,
                  bj.status, bj.trigger_type,
                  bj.started_at, bj.finished_at, bj.created_at,
                  u.username AS triggered_by_name
           FROM build_jobs bj
           LEFT JOIN users u ON u.id = bj.triggered_by
           WHERE bj.app_id = ?
           ORDER BY bj.created_at DESC
           LIMIT 50"#,
        app_id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!(rows.iter().map(|r| serde_json::json!({
        "id":               r.id,
        "k8s_job_name":     r.k8s_job_name,
        "git_commit_hash":  r.git_commit_hash,
        "image_tag":        r.image_tag,
        "status":           r.status,
        "trigger_type":     r.trigger_type,
        "triggered_by":     r.triggered_by_name,
        "started_at":       r.started_at,
        "finished_at":      r.finished_at,
        "created_at":       r.created_at,
    })).collect::<Vec<_>>())))
}

// ─── Get single build ─────────────────────────────────────────────────────────

/// GET /projects/:pid/apps/:aid/builds/:bid
pub async fn get_build(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, _app_id, build_id)): Path<(String, String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;

    let r = sqlx::query!(
        r#"SELECT bj.id, bj.app_id, bj.k8s_job_name, bj.git_commit_hash, bj.image_tag,
                  bj.status, bj.trigger_type,
                  bj.started_at, bj.finished_at, bj.created_at,
                  u.username AS triggered_by_name
           FROM build_jobs bj
           LEFT JOIN users u ON u.id = bj.triggered_by
           WHERE bj.id = ?"#,
        build_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("build {build_id}")))?;

    Ok(Json(serde_json::json!({
        "id":               r.id,
        "app_id":           r.app_id,
        "k8s_job_name":     r.k8s_job_name,
        "git_commit_hash":  r.git_commit_hash,
        "image_tag":        r.image_tag,
        "status":           r.status,
        "trigger_type":     r.trigger_type,
        "triggered_by":     r.triggered_by_name,
        "started_at":       r.started_at,
        "finished_at":      r.finished_at,
        "created_at":       r.created_at,
    })))
}

// ─── Trigger build ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TriggerBuildRequest {
    /// Git branch to build (overrides app default)
    pub branch: Option<String>,
}

/// POST /projects/:pid/apps/:aid/builds
pub async fn trigger_build(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, app_id)): Path<(String, String)>,
    Json(body): Json<TriggerBuildRequest>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;

    // Verify the app exists and is GIT-sourced
    let app = sqlx::query!(
        r#"SELECT source_type, git_url, git_branch FROM apps WHERE id = ? AND project_id = ?"#,
        app_id, project_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("app {app_id}")))?;

    if app.source_type != "GIT" {
        return Err(AppError::BadRequest(
            "only GIT-source apps can be built".into()
        ));
    }
    if app.git_url.is_none() {
        return Err(AppError::BadRequest("app has no git_url configured".into()));
    }

    // Reject if a build is already in progress
    let running = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM build_jobs WHERE app_id = ? AND status IN ('PENDING','RUNNING')"#,
        app_id
    )
    .fetch_one(&state.db)
    .await?;

    if running > 0 {
        return Err(AppError::Conflict("a build is already in progress for this app".into()));
    }

    let branch = body.branch
        .or(app.git_branch)
        .unwrap_or_else(|| "main".into());

    let build_id = Uuid::new_v4().to_string();
    let job_name = format!("qs-build-{}", &build_id[..8]);

    sqlx::query!(
        r#"INSERT INTO build_jobs (id, app_id, k8s_job_name, trigger_type, triggered_by)
           VALUES (?, ?, ?, 'MANUAL', ?)"#,
        build_id, app_id, job_name, auth.user_id,
    )
    .execute(&state.db)
    .await?;

    // Spawn background task to run the build (K8s Job or Docker container).
    let state2 = state.clone();
    let app_id2 = app_id.clone();
    let build_id2 = build_id.clone();
    let job_name2 = job_name.clone();
    let branch2 = branch.clone();
    tokio::spawn(async move {
        if let Err(e) = run_build_dispatch(&state2, &app_id2, &build_id2, &job_name2, &branch2).await {
            tracing::error!("build {build_id2} failed to start: {e}");
            let _ = sqlx::query!(
                r#"UPDATE build_jobs SET status = 'FAILED', finished_at = NOW() WHERE id = ?"#,
                build_id2
            )
            .execute(&state2.db)
            .await;
        }
    });

    Ok((axum::http::StatusCode::CREATED, Json(serde_json::json!({
        "id":           build_id,
        "k8s_job_name": job_name,
        "status":       "PENDING",
    }))))
}

// ─── Cancel build ─────────────────────────────────────────────────────────────

/// DELETE /projects/:pid/apps/:aid/builds/:bid
pub async fn cancel_build(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, _app_id, build_id)): Path<(String, String, String)>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OPERATOR").await?;

    #[derive(sqlx::FromRow)]
    struct CancelRow {
        status: String,
        k8s_job_name: Option<String>,
        node_id: Option<String>,
        container_id: Option<String>,
    }
    let build: CancelRow = sqlx::query_as(
        "SELECT status, k8s_job_name, node_id, container_id FROM build_jobs WHERE id = ?",
    )
    .bind(&build_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("build {build_id}")))?;

    if !matches!(build.status.as_str(), "PENDING" | "RUNNING") {
        return Err(AppError::Conflict(
            format!("build is already {}", build.status)
        ));
    }

    sqlx::query!(
        r#"UPDATE build_jobs SET status = 'CANCELLED', finished_at = NOW() WHERE id = ?"#,
        build_id
    )
    .execute(&state.db)
    .await?;

    // Best-effort: tear down the in-flight build (K8s Job or Docker container).
    #[derive(sqlx::FromRow)]
    struct AppRow {
        pool_id: Option<String>,
        namespace: String,
    }
    let app_row: Option<AppRow> = sqlx::query_as(
        "SELECT a.pool_id, p.name AS namespace \
         FROM build_jobs bj \
         JOIN apps a ON a.id = bj.app_id \
         JOIN projects p ON p.id = a.project_id \
         WHERE bj.id = ?",
    )
    .bind(&build_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some(row) = app_row {
        if let Some(pool_id) = row.pool_id {
            if let Ok(cluster_id) = resolve_cluster(&state, &pool_id).await {
                let orch = cluster_orchestrator(&state, &cluster_id)
                    .await
                    .unwrap_or_else(|_| "K3S".to_string());
                if orch == "DOCKER" {
                    #[derive(sqlx::FromRow)]
                    struct NodeAddr {
                        ip_address: String,
                        agent_port: u16,
                    }
                    if let (Some(node_id), Some(cid)) = (build.node_id, build.container_id) {
                        if let Ok(Some(node)) = sqlx::query_as::<_, NodeAddr>(
                            "SELECT ip_address, agent_port FROM cluster_nodes WHERE id = ?",
                        )
                        .bind(&node_id)
                        .fetch_optional(&state.db)
                        .await
                        {
                            let agent_token =
                                crate::docker::deployment::load_agent_token(&state.db).await;
                            let agent =
                                crate::docker::agent_client::AgentClient::new(&agent_token);
                            let _ = agent
                                .stop_container(&node.ip_address, node.agent_port, &cid)
                                .await;
                            let _ = agent
                                .remove_container(&node.ip_address, node.agent_port, &cid)
                                .await;
                        }
                    }
                } else if let Some(job_name) = build.k8s_job_name {
                    let _ = delete_k8s_job(&state, &cluster_id, &row.namespace, &job_name).await;
                }
            }
        }
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Build logs (SSE stream) ──────────────────────────────────────────────────

/// GET /projects/:pid/apps/:aid/builds/:bid/logs
pub async fn build_logs(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path((project_id, _app_id, build_id)): Path<(String, String, String)>,
) -> AppResult<axum::response::Response> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;

    #[derive(sqlx::FromRow)]
    struct BuildLogRow {
        k8s_job_name: Option<String>,
        status: String,
        node_id: Option<String>,
        container_id: Option<String>,
        pool_id: Option<String>,
        namespace: String,
    }
    let build: BuildLogRow = sqlx::query_as(
        "SELECT bj.k8s_job_name, bj.status, bj.node_id, bj.container_id, \
                a.pool_id, p.name AS namespace \
         FROM build_jobs bj \
         JOIN apps a ON a.id = bj.app_id \
         JOIN projects p ON p.id = a.project_id \
         WHERE bj.id = ?",
    )
    .bind(&build_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("build {build_id}")))?;

    let pool_id = build.pool_id
        .ok_or_else(|| AppError::BadRequest("app has no pool assigned".into()))?;
    let cluster_id = resolve_cluster(&state, &pool_id).await?;

    // ── Docker: proxy the build container's logs from its node's agent ────────
    if cluster_orchestrator(&state, &cluster_id).await? == "DOCKER" {
        let container_id = build
            .container_id
            .ok_or_else(|| AppError::NotFound("build container not started yet".into()))?;
        let node_id = build
            .node_id
            .ok_or_else(|| AppError::BadRequest("build has no node assigned".into()))?;
        #[derive(sqlx::FromRow)]
        struct NodeAddr {
            ip_address: String,
            agent_port: u16,
        }
        let node: NodeAddr = sqlx::query_as(
            "SELECT ip_address, agent_port FROM cluster_nodes WHERE id = ?",
        )
        .bind(&node_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("build node not found".into()))?;

        let agent_token = crate::docker::deployment::load_agent_token(&state.db).await;
        let url = format!(
            "http://{}:{}/containers/{}/logs?tail=500&follow=true",
            node.ip_address, node.agent_port, container_id
        );
        let resp = reqwest::Client::new()
            .get(&url)
            .bearer_auth(&agent_token)
            .send()
            .await
            .map_err(|e| AppError::Docker(format!("build log stream: {e}")))?;
        let byte_stream = resp.bytes_stream();

        use async_stream::stream;
        use axum::response::sse::Event;
        use futures::StreamExt;
        let sse_stream = stream! {
            futures::pin_mut!(byte_stream);
            while let Some(chunk) = byte_stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        for line in text.lines() {
                            if let Some(data) = line.strip_prefix("data:") {
                                yield Ok::<_, std::convert::Infallible>(
                                    Event::default().data(data.trim()),
                                );
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        };
        return Ok(Sse::new(sse_stream).into_response());
    }

    // ── K8s: stream from the build pod ────────────────────────────────────────
    let job_name = build.k8s_job_name
        .ok_or_else(|| AppError::BadRequest("build has no K8s job name".into()))?;

    let kube = crate::k8s::client_for_cluster(&state, &cluster_id).await?;

    use k8s_openapi::api::core::v1::Pod;
    use kube::Api;
    use kube::api::LogParams;
    use axum::response::sse::Event;
    use async_stream::stream;
    use futures::io::AsyncBufReadExt;

    let pods: Api<Pod> = Api::namespaced(kube, &build.namespace);
    let lp = kube::api::ListParams::default()
        .labels(&format!("job-name={job_name}"));

    let pod_list = pods.list(&lp).await
        .map_err(|e| AppError::Kubernetes(e))?;

    let pod_name = pod_list.items.into_iter()
        .next()
        .and_then(|p| p.metadata.name)
        .ok_or_else(|| AppError::NotFound("build pod not found yet".into()))?;

    let follow = build.status == "RUNNING";
    let log_reader = pods
        .log_stream(&pod_name, &LogParams { follow, tail_lines: Some(500), ..Default::default() })
        .await
        .map_err(|e| AppError::Kubernetes(e))?;

    let sse_stream = stream! {
        let mut lines = log_reader.lines();
        loop {
            match futures::StreamExt::next(&mut lines).await {
                Some(Ok(line)) => {
                    yield Ok::<_, std::convert::Infallible>(
                        Event::default().data(line)
                    );
                }
                _ => break,
            }
        }
    };

    Ok(Sse::new(sse_stream).into_response())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

async fn resolve_cluster(state: &AppState, pool_id: &str) -> AppResult<String> {
    sqlx::query_scalar!(
        r#"SELECT id FROM clusters WHERE pool_id = ? AND is_active = 1 ORDER BY created_at LIMIT 1"#,
        pool_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest(format!("no active cluster in pool {pool_id}")))
}

async fn cluster_orchestrator(state: &AppState, cluster_id: &str) -> AppResult<String> {
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT orchestrator FROM clusters WHERE id = ?",
    )
    .bind(cluster_id)
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_else(|| "K3S".to_string()))
}

async fn load_cfg(state: &AppState, key: &str) -> String {
    sqlx::query_scalar::<_, String>("SELECT `value` FROM platform_config WHERE `key` = ?")
        .bind(key)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// Resolve the app's cluster + orchestrator and run the build on the right backend.
async fn run_build_dispatch(
    state: &AppState,
    app_id: &str,
    build_id: &str,
    job_name: &str,
    branch: &str,
) -> AppResult<()> {
    let pool_id = sqlx::query_scalar::<_, Option<String>>("SELECT pool_id FROM apps WHERE id = ?")
        .bind(app_id)
        .fetch_optional(&state.db)
        .await?
        .flatten()
        .ok_or_else(|| AppError::BadRequest("app has no pool assigned".into()))?;
    let cluster_id = resolve_cluster(state, &pool_id).await?;

    if cluster_orchestrator(state, &cluster_id).await? == "DOCKER" {
        run_build_container(state, app_id, build_id, &cluster_id, branch).await
    } else {
        run_build_job(state, app_id, build_id, job_name, branch).await
    }
}

// ─── Internal: run the build as a kaniko container on a Docker node ───────────

/// Build a docker `config.json` auth blob for the push registry, or None when
/// no registry credentials are configured (anonymous push).
async fn build_registry_config_json(state: &AppState, registry: &str) -> Option<String> {
    let user = load_cfg(state, "registry_username").await;
    let pass_raw = load_cfg(state, "registry_password").await;
    if registry.is_empty() || user.is_empty() || pass_raw.is_empty() {
        return None;
    }
    // Password may be stored encrypted; fall back to the raw value otherwise.
    let pass = state.crypto.decrypt(&pass_raw).unwrap_or(pass_raw);
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let auth = B64.encode(format!("{user}:{pass}"));
    Some(serde_json::json!({ "auths": { registry: { "auth": auth } } }).to_string())
}

async fn run_build_container(
    state: &AppState,
    app_id: &str,
    build_id: &str,
    cluster_id: &str,
    branch: &str,
) -> AppResult<()> {
    use crate::docker::agent_client::{AgentClient, FileMount, RunContainerRequest};

    #[derive(sqlx::FromRow)]
    struct BuildApp {
        app_name: String,
        git_url: Option<String>,
        git_token: Option<String>,
    }
    let app: BuildApp = sqlx::query_as(
        "SELECT name AS app_name, git_url, git_token FROM apps WHERE id = ?",
    )
    .bind(app_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("app {app_id}")))?;

    let git_url = app
        .git_url
        .ok_or_else(|| AppError::BadRequest("app has no git_url".into()))?;

    // Destinations: push to the ref the deploy path pulls (:latest) + a build tag.
    let registry = load_cfg(state, "registry_host").await;
    let latest_ref = format!("{}/{}:latest", registry, app.app_name);
    let build_ref = format!("{}/{}:build-{}", registry, app.app_name, &build_id[..8]);
    let insecure = load_cfg(state, "registry_insecure").await == "1";

    let mut args = vec![
        format!("--context=git://{git_url}#refs/heads/{branch}"),
        format!("--destination={latest_ref}"),
        format!("--destination={build_ref}"),
        "--cache=true".to_string(),
        "--cache-ttl=24h".to_string(),
    ];
    if insecure {
        args.push("--insecure".to_string());
        args.push("--skip-tls-verify".to_string());
    }

    // Optional registry auth for the push, mounted where kaniko expects it.
    let mut file_mounts: Vec<FileMount> = Vec::new();
    if let Some(content) = build_registry_config_json(state, &registry).await {
        file_mounts.push(FileMount {
            filename: "config.json".to_string(),
            mount_path: "/kaniko/.docker/config.json".to_string(),
            content,
        });
    }

    // Git auth for private repos (kaniko reads GIT_USERNAME / GIT_PASSWORD).
    let mut env: Vec<(String, String)> = Vec::new();
    if let Some(tok) = app.git_token {
        if !tok.is_empty() {
            let token = state.crypto.decrypt(&tok).unwrap_or(tok);
            env.push(("GIT_USERNAME".to_string(), "oauth2".to_string()));
            env.push(("GIT_PASSWORD".to_string(), token));
        }
    }

    // One node, small resource caps so builds don't starve app workloads.
    let targets =
        crate::docker::scheduler::pick_nodes(state, cluster_id, 1, Some(2000), Some(4096), false, 0)
            .await?;
    let target = targets
        .first()
        .ok_or_else(|| AppError::BadRequest("no eligible Docker node for build".into()))?;

    let agent_token = crate::docker::deployment::load_agent_token(&state.db).await;
    let agent = AgentClient::new(&agent_token);
    let container_name = format!("qs-build-{}", &build_id[..8]);

    let req = RunContainerRequest {
        container_name,
        image: "gcr.io/kaniko-project/executor:latest".to_string(),
        command: None,
        args: Some(args),
        working_dir: None,
        env,
        cpu_limit_mcores: Some(2000),
        mem_limit_mb: Some(4096),
        gpu_count: 0,
        network_name: None,
        ip_address: None,
        extra_networks: Vec::new(),
        port_bindings: Vec::new(),
        volumes: Vec::new(),
        file_mounts,
        health_check: None,
        restart_policy: "no".to_string(),
        user: None,
        privileged: false,
        read_only_rootfs: false,
        registry_auth: None,
    };

    let resp = agent
        .run_container(&target.ip_address, target.agent_port, &req)
        .await?;

    sqlx::query(
        "UPDATE build_jobs \
         SET status = 'RUNNING', image_tag = ?, node_id = ?, container_id = ?, started_at = NOW() \
         WHERE id = ?",
    )
    .bind(&latest_ref)
    .bind(&target.node_id)
    .bind(&resp.container_id)
    .bind(build_id)
    .execute(&state.db)
    .await?;

    let state2 = state.clone();
    let build_id2 = build_id.to_string();
    let ip = target.ip_address.clone();
    let port = target.agent_port;
    let cid = resp.container_id.clone();
    tokio::spawn(async move {
        watch_build_completion_docker(&state2, &ip, port, &cid, &build_id2).await;
    });

    Ok(())
}

async fn watch_build_completion_docker(
    state: &AppState,
    node_ip: &str,
    agent_port: u16,
    container_id: &str,
    build_id: &str,
) {
    use crate::docker::agent_client::AgentClient;
    use tokio::time::{sleep, Duration};

    let agent_token = crate::docker::deployment::load_agent_token(&state.db).await;
    let agent = AgentClient::new(&agent_token);

    // Poll up to 60 minutes (240 × 15s).
    let mut final_status: Option<&str> = None;
    for _ in 0..240 {
        sleep(Duration::from_secs(15)).await;
        match agent.inspect(node_ip, agent_port, container_id).await {
            Ok(info) if info.exited => {
                final_status = Some(if info.exit_code == 0 { "SUCCEEDED" } else { "FAILED" });
                break;
            }
            Ok(_) => {} // still running
            Err(e) => {
                tracing::warn!(build_id, "build inspect error: {e}");
            }
        }
    }
    let status = final_status.unwrap_or("FAILED");

    let _ = sqlx::query("UPDATE build_jobs SET status = ?, finished_at = NOW() WHERE id = ?")
        .bind(status)
        .bind(build_id)
        .execute(&state.db)
        .await;

    // Best-effort cleanup of the (now-exited) build container.
    let _ = agent.remove_container(node_ip, agent_port, container_id).await;
}

// ─── Internal: run the build Job in K8s ──────────────────────────────────────

async fn run_build_job(
    state: &AppState,
    app_id: &str,
    build_id: &str,
    job_name: &str,
    branch: &str,
) -> AppResult<()> {
    use k8s_openapi::api::batch::v1::{Job, JobSpec};
    use k8s_openapi::api::core::v1::{Container, EnvVar, PodSpec, PodTemplateSpec};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use kube::{Api, api::PostParams};
    use std::collections::BTreeMap;

    #[derive(sqlx::FromRow)]
    struct BuildJobApp {
        app_name: String,
        git_url: Option<String>,
        pool_id: Option<String>,
        namespace: String,
    }
    let app: BuildJobApp = sqlx::query_as(
        "SELECT a.name AS app_name, a.git_url, a.pool_id, p.name AS namespace \
         FROM apps a \
         JOIN projects p ON p.id = a.project_id \
         WHERE a.id = ?",
    )
    .bind(app_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("app {app_id}")))?;

    let pool_id = app.pool_id
        .ok_or_else(|| AppError::BadRequest("app has no pool assigned".into()))?;
    let cluster_id = resolve_cluster(state, &pool_id).await?;

    let kube = crate::k8s::client_for_cluster(state, &cluster_id).await?;

    let git_url = app.git_url
        .ok_or_else(|| AppError::BadRequest("app has no git_url".into()))?;

    // Push to the ref the deploy path pulls (:latest) + a build tag, so a GIT
    // deploy actually finds the produced image (parity with the Docker builder).
    let registry = load_cfg(state, "registry_host").await;
    let image_tag = format!("{}/{}:latest", registry, app.app_name);
    let build_ref = format!("{}/{}:build-{}", registry, app.app_name, &build_id[..8]);

    let mut labels = BTreeMap::new();
    labels.insert("qs-build-id".to_string(), build_id[..8].to_string());
    labels.insert("job-name".to_string(), job_name.to_string());

    let job = Job {
        metadata: ObjectMeta {
            name: Some(job_name.to_string()),
            namespace: Some(app.namespace.clone()),
            labels: Some(labels.clone()),
            ..Default::default()
        },
        spec: Some(JobSpec {
            backoff_limit: Some(0),
            ttl_seconds_after_finished: Some(3600),
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(labels),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    restart_policy: Some("Never".into()),
                    containers: vec![Container {
                        name: "builder".to_string(),
                        image: Some("gcr.io/kaniko-project/executor:latest".to_string()),
                        args: Some(vec![
                            format!("--context=git://{git_url}#refs/heads/{branch}"),
                            format!("--destination={image_tag}"),
                            format!("--destination={build_ref}"),
                            "--cache=true".to_string(),
                            "--cache-ttl=24h".to_string(),
                        ]),
                        env: Some(vec![
                            EnvVar {
                                name: "GIT_BRANCH".to_string(),
                                value: Some(branch.to_string()),
                                ..Default::default()
                            },
                        ]),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
            },
            ..Default::default()
        }),
        ..Default::default()
    };

    let jobs: Api<Job> = Api::namespaced(kube, &app.namespace);
    jobs.create(&PostParams::default(), &job).await
        .map_err(|e| AppError::Kubernetes(e))?;

    sqlx::query!(
        r#"UPDATE build_jobs SET status = 'RUNNING', image_tag = ?, started_at = NOW() WHERE id = ?"#,
        image_tag, build_id,
    )
    .execute(&state.db)
    .await?;

    let state2 = state.clone();
    let build_id2 = build_id.to_string();
    let namespace = app.namespace.clone();
    let job_name2 = job_name.to_string();
    tokio::spawn(async move {
        watch_build_completion(&state2, &cluster_id, &namespace, &job_name2, &build_id2).await;
    });

    Ok(())
}

async fn watch_build_completion(
    state: &AppState,
    cluster_id: &str,
    namespace: &str,
    job_name: &str,
    build_id: &str,
) {
    use k8s_openapi::api::batch::v1::Job;
    use kube::Api;
    use tokio::time::{sleep, Duration};

    let kube = match crate::k8s::client_for_cluster(state, cluster_id).await {
        Ok(c) => c,
        Err(_) => return,
    };

    let jobs: Api<Job> = Api::namespaced(kube, namespace);

    for _ in 0..120 {
        sleep(Duration::from_secs(30)).await;

        let job = match jobs.get(job_name).await {
            Ok(j) => j,
            Err(_) => break,
        };

        let conditions = job.status
            .as_ref()
            .and_then(|s| s.conditions.as_ref())
            .cloned()
            .unwrap_or_default();

        let succeeded = conditions.iter().any(|c| c.type_ == "Complete" && c.status == "True");
        let failed = conditions.iter().any(|c| c.type_ == "Failed" && c.status == "True");

        if succeeded {
            let _ = sqlx::query!(
                r#"UPDATE build_jobs SET status = 'SUCCEEDED', finished_at = NOW() WHERE id = ?"#,
                build_id
            )
            .execute(&state.db)
            .await;
            return;
        }
        if failed {
            let _ = sqlx::query!(
                r#"UPDATE build_jobs SET status = 'FAILED', finished_at = NOW() WHERE id = ?"#,
                build_id
            )
            .execute(&state.db)
            .await;
            return;
        }
    }

    // Timed out after 60 minutes
    let _ = sqlx::query!(
        r#"UPDATE build_jobs SET status = 'FAILED', finished_at = NOW() WHERE id = ?"#,
        build_id
    )
    .execute(&state.db)
    .await;
}

async fn delete_k8s_job(
    state: &AppState,
    cluster_id: &str,
    namespace: &str,
    job_name: &str,
) -> AppResult<()> {
    use k8s_openapi::api::batch::v1::Job;
    use kube::{Api, api::DeleteParams};

    let kube = crate::k8s::client_for_cluster(state, cluster_id).await?;
    let jobs: Api<Job> = Api::namespaced(kube, namespace);
    let mut dp = DeleteParams::default();
    dp.propagation_policy = Some(kube::api::PropagationPolicy::Background);
    let _ = jobs.delete(job_name, &dp).await;
    Ok(())
}
