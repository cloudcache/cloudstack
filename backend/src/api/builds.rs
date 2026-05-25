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

    // Spawn background task to run the K8s build Job
    let state2 = state.clone();
    let app_id2 = app_id.clone();
    let build_id2 = build_id.clone();
    let job_name2 = job_name.clone();
    tokio::spawn(async move {
        if let Err(e) = run_build_job(&state2, &app_id2, &build_id2, &job_name2, &branch).await {
            tracing::error!("build job {build_id2} failed to start: {e}");
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

    let build = sqlx::query!(
        r#"SELECT status, k8s_job_name FROM build_jobs WHERE id = ?"#,
        build_id
    )
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

    // Best-effort: delete the K8s Job
    if let Some(job_name) = build.k8s_job_name {
        let app_row = sqlx::query!(
            r#"SELECT a.pool_id, p.name AS namespace
               FROM build_jobs bj
               JOIN apps a ON a.id = bj.app_id
               JOIN projects p ON p.id = a.project_id
               WHERE bj.id = ?"#,
            build_id
        )
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if let Some(row) = app_row {
            if let Some(pool_id) = row.pool_id {
                if let Ok(cluster_id) = resolve_cluster(&state, &pool_id).await {
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
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;

    let build = sqlx::query!(
        r#"SELECT bj.k8s_job_name, bj.status,
                  a.pool_id, p.name AS namespace
           FROM build_jobs bj
           JOIN apps a ON a.id = bj.app_id
           JOIN projects p ON p.id = a.project_id
           WHERE bj.id = ?"#,
        build_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("build {build_id}")))?;

    let pool_id = build.pool_id
        .ok_or_else(|| AppError::BadRequest("app has no pool assigned".into()))?;
    let cluster_id = resolve_cluster(&state, &pool_id).await?;

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

    Ok(Sse::new(sse_stream))
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

    let app = sqlx::query!(
        r#"SELECT a.git_url, a.container_image, a.pool_id,
                  p.name AS namespace
           FROM apps a
           JOIN projects p ON p.id = a.project_id
           WHERE a.id = ?"#,
        app_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("app {app_id}")))?;

    let pool_id = app.pool_id
        .ok_or_else(|| AppError::BadRequest("app has no pool assigned".into()))?;
    let cluster_id = resolve_cluster(state, &pool_id).await?;

    let kube = crate::k8s::client_for_cluster(state, &cluster_id).await?;

    let git_url = app.git_url
        .ok_or_else(|| AppError::BadRequest("app has no git_url".into()))?;

    let image_tag = format!(
        "{}:build-{}",
        app.container_image.unwrap_or_else(|| format!("localhost/qs/{app_id}")),
        &build_id[..8]
    );

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
