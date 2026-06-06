//! Per-project usage + quota for managed-service bindings (P2c).
//!
//! Tracks how many distinct managed resources each project is using of
//! every kind shipped in P2:
//!   - database_instance   (provisioned-or-bound DBs)
//!   - mq_endpoint         (RabbitMQ etc.)
//!   - smtp_endpoint       (mail relays)
//!   - redis_endpoint      (cache)
//!   - s3_target           (object storage)
//!
//! Usage counts come from `app_template_bindings JOIN apps`; quotas live on
//! `projects.quota_*`. Two surfaces:
//!   - `check_binding_allowed()` is called from templates_deploy BEFORE
//!     creating new bindings; rejects with QuotaExceeded if it would push
//!     usage over the per-project limit.
//!   - `GET /projects/:pid/managed-usage` returns the full breakdown for UIs.

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Extension, Json,
};

use crate::{
    auth::middleware::AuthUser,
    error::{AppError, AppResult},
    state::AppState,
};

/// One {used, limit} pair per kind. `limit == 0` means unlimited.
#[derive(serde::Serialize)]
pub struct KindUsage {
    pub used: i64,
    pub limit: i64,
}

#[derive(serde::Serialize)]
pub struct ManagedUsage {
    pub db_instances: KindUsage,
    pub mq_bindings: KindUsage,
    pub smtp_bindings: KindUsage,
    pub redis_bindings: KindUsage,
    pub s3_bindings: KindUsage,
}

/// GET /api/v1/projects/:project_id/managed-usage
pub async fn get_managed_usage(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(project_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "OBSERVER").await?;
    let usage = compute_managed_usage(&state, &project_id).await?;
    Ok(Json(serde_json::json!({
        "db_instances":   { "used": usage.db_instances.used,   "limit": usage.db_instances.limit },
        "mq_bindings":    { "used": usage.mq_bindings.used,    "limit": usage.mq_bindings.limit },
        "smtp_bindings":  { "used": usage.smtp_bindings.used,  "limit": usage.smtp_bindings.limit },
        "redis_bindings": { "used": usage.redis_bindings.used, "limit": usage.redis_bindings.limit },
        "s3_bindings":    { "used": usage.s3_bindings.used,    "limit": usage.s3_bindings.limit },
    })))
}

pub async fn compute_managed_usage(state: &AppState, project_id: &str) -> AppResult<ManagedUsage> {
    #[derive(sqlx::FromRow)]
    struct Limits {
        quota_db_instances: u32,
        quota_mq_bindings: u32,
        quota_smtp_bindings: u32,
        quota_redis_bindings: u32,
        quota_s3_bindings: u32,
    }
    let limits: Limits = sqlx::query_as(
        "SELECT quota_db_instances, quota_mq_bindings, quota_smtp_bindings, \
                quota_redis_bindings, quota_s3_bindings \
         FROM projects WHERE id = ?",
    )
    .bind(project_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("project {project_id}")))?;

    // Provisioned/owned DBs live in database_instances regardless of bindings.
    let db_used: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM database_instances WHERE project_id = ?",
    )
    .bind(project_id)
    .fetch_one(&state.db)
    .await?;

    // Bindings of every other kind come from app_template_bindings JOIN apps.
    // We count DISTINCT (binding_ref_id) per kind so a project that binds the
    // same MQ from two apps is charged once.
    async fn distinct_bindings(
        state: &AppState, project_id: &str, kind: &str,
    ) -> AppResult<i64> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(DISTINCT b.binding_ref_id) \
             FROM app_template_bindings b \
             JOIN apps a ON a.id = b.app_id \
             WHERE a.project_id = ? AND b.binding_kind = ?",
        )
        .bind(project_id)
        .bind(kind)
        .fetch_one(&state.db)
        .await?)
    }

    let mq_used = distinct_bindings(state, project_id, "mq_endpoint").await?;
    let smtp_used = distinct_bindings(state, project_id, "smtp_endpoint").await?;
    let redis_used = distinct_bindings(state, project_id, "redis_endpoint").await?;
    let s3_used = distinct_bindings(state, project_id, "s3_target").await?;

    Ok(ManagedUsage {
        db_instances:   KindUsage { used: db_used,    limit: limits.quota_db_instances    as i64 },
        mq_bindings:    KindUsage { used: mq_used,    limit: limits.quota_mq_bindings     as i64 },
        smtp_bindings:  KindUsage { used: smtp_used,  limit: limits.quota_smtp_bindings   as i64 },
        redis_bindings: KindUsage { used: redis_used, limit: limits.quota_redis_bindings  as i64 },
        s3_bindings:    KindUsage { used: s3_used,    limit: limits.quota_s3_bindings     as i64 },
    })
}

/// Pre-deploy quota check. `incoming` is a map of binding_kind → number of
/// new distinct refs this deploy would introduce.
///
/// Provisioned-new resources always cost 1; managed (existing) bindings only
/// cost extra if this project isn't already bound to that ref. Caller fills
/// the map by checking against `app_template_bindings` for each requirement.
pub async fn check_binding_allowed(
    state: &AppState,
    project_id: &str,
    incoming: &std::collections::HashMap<String, i64>,
) -> AppResult<()> {
    let usage = compute_managed_usage(state, project_id).await?;
    for (kind, delta) in incoming {
        let (used, limit) = match kind.as_str() {
            "database_instance" => (usage.db_instances.used,   usage.db_instances.limit),
            "mq_endpoint"       => (usage.mq_bindings.used,    usage.mq_bindings.limit),
            "smtp_endpoint"     => (usage.smtp_bindings.used,  usage.smtp_bindings.limit),
            "redis_endpoint"    => (usage.redis_bindings.used, usage.redis_bindings.limit),
            "s3_target"         => (usage.s3_bindings.used,    usage.s3_bindings.limit),
            _ => continue,
        };
        if limit > 0 && used + *delta > limit {
            return Err(AppError::QuotaExceeded(format!(
                "{kind} quota would be exceeded: used={used} + new={delta} > limit={limit}"
            )));
        }
    }
    Ok(())
}
