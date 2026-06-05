/// Quota enforcement for user apps.
///
/// Quota dimensions live on `projects` (cpu_mcores, mem_mb, app_count).
/// Usage is the SUM over apps in status RUNNING / DEPLOYING / PAUSED
/// (paused apps still hold their resource reservation).
/// SUSPENDED apps do NOT count — suspension is how the system frees quota.
///
/// Flow:
///   deploy/scale/resume → check_deploy_allowed() → reject with 429 if hard limit exceeded
///   background task     → enforce_project() → suspend running apps until under limit
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};

// ── Threshold defaults (overridable via platform_config) ──────────────────────

const DEFAULT_WARN_PCT: f32 = 80.0;
const DEFAULT_HARD_PCT: f32 = 100.0;

pub struct QuotaThresholds {
    pub warn_pct: f32,
    pub hard_pct: f32,
    pub enabled: bool,
}

pub async fn load_thresholds(state: &AppState) -> QuotaThresholds {
    let get = |key: &str, default: &str| {
        let db = state.db.clone();
        let k = key.to_string();
        let d = default.to_string();
        async move {
            sqlx::query_scalar!(r#"SELECT `value` FROM platform_config WHERE `key` = ?"#, k)
                .fetch_optional(&db)
                .await
                .ok()
                .flatten()
                .unwrap_or(d)
        }
    };
    QuotaThresholds {
        warn_pct: get("quota_warn_pct", "80")
            .await
            .parse()
            .unwrap_or(DEFAULT_WARN_PCT),
        hard_pct: get("quota_hard_pct", "100")
            .await
            .parse()
            .unwrap_or(DEFAULT_HARD_PCT),
        enabled: get("quota_check_enabled", "1").await != "0",
    }
}

// ── Data structs ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ProjectUsage {
    pub cpu_mcores: i64,
    pub mem_mb: i64,
    pub app_count: i64,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectQuota {
    /// 0 = unlimited
    pub cpu_mcores: i64,
    pub mem_mb: i64,
    pub app_count: i64,
}

#[derive(Debug, Clone)]
pub struct QuotaStatus {
    pub usage: ProjectUsage,
    pub quota: ProjectQuota,
    /// 0.0–100.0+ ; NaN when quota is 0 (unlimited)
    pub cpu_pct: f32,
    pub mem_pct: f32,
    pub app_pct: f32,
    pub cpu_warned: bool,
    pub mem_warned: bool,
    pub app_warned: bool,
    pub cpu_exceeded: bool,
    pub mem_exceeded: bool,
    pub app_exceeded: bool,
}

impl QuotaStatus {
    pub fn any_exceeded(&self) -> bool {
        self.cpu_exceeded || self.mem_exceeded || self.app_exceeded
    }
    pub fn any_warned(&self) -> bool {
        self.cpu_warned || self.mem_warned || self.app_warned
    }
}

fn pct(used: i64, limit: i64) -> f32 {
    if limit == 0 {
        return 0.0;
    } // 0 = unlimited
    (used as f32 / limit as f32) * 100.0
}

// ── Core computations ─────────────────────────────────────────────────────────

/// Current resource consumption by active apps (RUNNING + DEPLOYING + PAUSED).
pub async fn compute_usage(state: &AppState, project_id: &str) -> AppResult<ProjectUsage> {
    let row = sqlx::query!(
        r#"SELECT
             COALESCE(SUM(
               COALESCE(cpu_limit_mcores, cpu_reservation_mcores, 0) * COALESCE(replicas, 1)
             ), 0) AS cpu_mcores,
             COALESCE(SUM(
               COALESCE(mem_limit_mb, mem_reservation_mb, 0) * COALESCE(replicas, 1)
             ), 0) AS mem_mb,
             COUNT(*) AS app_count
           FROM apps
           WHERE project_id = ?
             AND status IN ('RUNNING', 'DEPLOYING', 'PAUSED')"#,
        project_id
    )
    .fetch_one(&state.db)
    .await?;

    use rust_decimal::prelude::ToPrimitive;
    Ok(ProjectUsage {
        cpu_mcores: row.cpu_mcores.to_i64().unwrap_or(0),
        mem_mb: row.mem_mb.to_i64().unwrap_or(0),
        app_count: row.app_count,
    })
}

/// Quota limits configured on the project (0 = unlimited).
///
/// Source of truth is `projects.quota_*` columns. These are set by:
///   1. Subscription plan activation → `allocate_plan_to_default_project()`
///   2. Admin manual override → `PUT /admin/projects/:id`
/// Admin overrides are intentional and take precedence until the next
/// subscription change. The quota enforcer always reads from `projects`.
pub async fn compute_quota(state: &AppState, project_id: &str) -> AppResult<ProjectQuota> {
    let row = sqlx::query!(
        r#"SELECT quota_cpu_mcores, quota_mem_mb, quota_apps
           FROM projects WHERE id = ?"#,
        project_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("project {project_id}")))?;

    Ok(ProjectQuota {
        cpu_mcores: row.quota_cpu_mcores as i64,
        mem_mb: row.quota_mem_mb as i64,
        app_count: row.quota_apps as i64,
    })
}

pub async fn check(state: &AppState, project_id: &str) -> AppResult<QuotaStatus> {
    let thresholds = load_thresholds(state).await;
    let usage = compute_usage(state, project_id).await?;
    let quota = compute_quota(state, project_id).await?;

    let cpu_pct = pct(usage.cpu_mcores, quota.cpu_mcores);
    let mem_pct = pct(usage.mem_mb, quota.mem_mb);
    let app_pct = pct(usage.app_count, quota.app_count);

    let exceeded = |p: f32, limit: i64| thresholds.enabled && limit > 0 && p >= thresholds.hard_pct;
    let warned = |p: f32, limit: i64| {
        thresholds.enabled && limit > 0 && p >= thresholds.warn_pct && p < thresholds.hard_pct
    };

    Ok(QuotaStatus {
        cpu_warned: warned(cpu_pct, quota.cpu_mcores),
        mem_warned: warned(mem_pct, quota.mem_mb),
        app_warned: warned(app_pct, quota.app_count),
        cpu_exceeded: exceeded(cpu_pct, quota.cpu_mcores),
        mem_exceeded: exceeded(mem_pct, quota.mem_mb),
        app_exceeded: exceeded(app_pct, quota.app_count),
        cpu_pct,
        mem_pct,
        app_pct,
        usage,
        quota,
    })
}

// ── Pre-deploy check ──────────────────────────────────────────────────────────

/// Called before deploy or scale-up. `extra_*` is the net additional resource requested.
/// Returns Err(QuotaExceeded) if the projected usage would breach the hard limit.
pub async fn check_deploy_allowed(
    state: &AppState,
    project_id: &str,
    extra_cpu_mcores: i64,
    extra_mem_mb: i64,
    extra_app_count: i64,
) -> AppResult<()> {
    let thresholds = load_thresholds(state).await;
    if !thresholds.enabled {
        return Ok(());
    }

    let usage = compute_usage(state, project_id).await?;
    let quota = compute_quota(state, project_id).await?;

    let check_dim = |dim: &str, used: i64, extra: i64, limit: i64| -> AppResult<()> {
        if limit == 0 {
            return Ok(());
        } // unlimited
        let projected_pct = pct(used + extra, limit);
        if projected_pct >= thresholds.hard_pct {
            return Err(AppError::QuotaExceeded(format!(
                "{dim} quota exceeded: {}/{} (projected {:.0}%)",
                used + extra,
                limit,
                projected_pct
            )));
        }
        Ok(())
    };

    check_dim("CPU", usage.cpu_mcores, extra_cpu_mcores, quota.cpu_mcores)?;
    check_dim("Memory", usage.mem_mb, extra_mem_mb, quota.mem_mb)?;
    check_dim("Apps", usage.app_count, extra_app_count, quota.app_count)?;

    Ok(())
}

// ── Suspension ────────────────────────────────────────────────────────────────

/// Suspends running apps in the project (largest consumers first) until
/// projected usage drops below the hard limit. Returns the list of suspended app names.
///
/// Called by: (a) background quota enforcer, (b) admin force-enforce endpoint.
pub async fn enforce_project(
    state: &AppState,
    project_id: &str,
    reason: &str,
) -> AppResult<Vec<String>> {
    let thresholds = load_thresholds(state).await;
    if !thresholds.enabled {
        return Ok(vec![]);
    }

    // Fetch RUNNING apps for this project, heaviest first
    let apps = sqlx::query!(
        r#"SELECT id, name,
             COALESCE(cpu_limit_mcores, cpu_reservation_mcores, 0) * COALESCE(replicas,1) AS cpu_cost,
             COALESCE(mem_limit_mb, mem_reservation_mb, 0) * COALESCE(replicas,1) AS mem_cost,
             replicas, pool_id
           FROM apps
           WHERE project_id = ? AND status = 'RUNNING'
           ORDER BY
             (COALESCE(cpu_limit_mcores, cpu_reservation_mcores, 0) * COALESCE(replicas,1)) DESC"#,
        project_id
    )
    .fetch_all(&state.db)
    .await?;

    let mut suspended_names: Vec<String> = Vec::new();

    for app in apps {
        // Re-check after each suspension (usage may have dropped enough)
        let status = check(state, project_id).await?;
        if !status.any_exceeded() {
            break;
        }

        // Scale K8s deployment to 0
        let suspend_result = suspend_app_k8s(state, project_id, &app.id, &app.name).await;
        if let Err(e) = suspend_result {
            tracing::warn!("quota enforcer: failed to scale down {}: {e}", app.name);
            continue;
        }

        // Mark SUSPENDED in DB
        sqlx::query!(
            r#"UPDATE apps
               SET status = 'SUSPENDED',
                   paused_at = NOW(),
                   paused_by = 'system',
                   pause_reason = ?
               WHERE id = ?"#,
            reason,
            app.id,
        )
        .execute(&state.db)
        .await?;

        record_violation(
            state,
            project_id,
            Some(&app.id),
            "cpu_mcores",
            app.cpu_cost,
            0,
            100.0,
            "suspend",
        )
        .await;

        suspended_names.push(app.name.clone());
        tracing::warn!(
            project_id, app_id = %app.id, app_name = %app.name,
            reason,
            "quota enforcer: app suspended"
        );
    }

    Ok(suspended_names)
}

async fn suspend_app_k8s(
    state: &AppState,
    project_id: &str,
    app_id: &str,
    app_name: &str,
) -> AppResult<()> {
    let ns = sqlx::query_scalar!(r#"SELECT name FROM projects WHERE id = ?"#, project_id)
        .fetch_one(&state.db)
        .await?;

    // Use the app's bound cluster_id (set by resolve_cluster_for_app on first deploy)
    let cluster_id = sqlx::query_scalar!(
        r#"SELECT COALESCE(cluster_id,
             (SELECT id FROM clusters
              WHERE pool_id = a.pool_id AND is_active = 1
              ORDER BY created_at LIMIT 1))
           FROM apps a WHERE a.id = ?"#,
        app_id
    )
    .fetch_optional(&state.db)
    .await?
    .flatten()
    .ok_or_else(|| AppError::Internal("no cluster for app".into()))?;

    crate::k8s::deployment::scale_deployment(state, &cluster_id, &ns, app_name, 0).await
}

// ── Violation log ─────────────────────────────────────────────────────────────

pub async fn record_violation(
    state: &AppState,
    project_id: &str,
    app_id: Option<&str>,
    dimension: &str,
    used: i64,
    quota: i64,
    pct_used: f32,
    action: &str,
) {
    let id = Uuid::new_v4().to_string();
    let _ = sqlx::query!(
        r#"INSERT INTO quota_violations
             (id, project_id, app_id, dimension, used_value, quota_value, pct_used, action)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
        id,
        project_id,
        app_id,
        dimension,
        used,
        quota,
        pct_used,
        action
    )
    .execute(&state.db)
    .await;
}

// ── Background enforcer ───────────────────────────────────────────────────────

/// Runs once: checks every project with active apps and suspends any that are over quota.
pub async fn run_enforcer(state: &AppState) -> AppResult<()> {
    let thresholds = load_thresholds(state).await;
    if !thresholds.enabled {
        return Ok(());
    }

    let projects = sqlx::query_scalar!(
        r#"SELECT DISTINCT project_id FROM apps
           WHERE status IN ('RUNNING', 'DEPLOYING', 'PAUSED')"#
    )
    .fetch_all(&state.db)
    .await?;

    for project_id in projects {
        let status = match check(state, &project_id).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(project_id, "quota enforcer: check failed: {e}");
                continue;
            }
        };

        if status.any_exceeded() {
            let suspended = enforce_project(state, &project_id, "quota_exceeded")
                .await
                .unwrap_or_default();
            if !suspended.is_empty() {
                tracing::warn!(project_id, apps = ?suspended, "quota enforcer: suspended apps");
            }
        } else if !status.any_warned() {
            resolve_violations(state, &project_id).await;
        } else if status.any_warned() {
            // Record warn event (non-blocking; ignore duplicate inserts via the PK)
            record_violation(
                state,
                &project_id,
                None,
                "app_count",
                status.usage.app_count,
                status.quota.app_count,
                status.app_pct,
                "warn",
            )
            .await;
        }
    }

    Ok(())
}

/// Mark any open violation records for this project/dimension as resolved.
pub async fn resolve_violations(state: &AppState, project_id: &str) {
    let _ = sqlx::query!(
        r#"UPDATE quota_violations
           SET resolved_at = NOW()
           WHERE project_id = ? AND resolved_at IS NULL"#,
        project_id
    )
    .execute(&state.db)
    .await;
}
