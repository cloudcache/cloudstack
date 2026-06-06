/// Subscription plan management + user subscription lifecycle.
///
/// Plans define a named quota bundle with a price.
/// Subscriptions link a user to a plan for a period.
/// When a subscription becomes ACTIVE, the plan quota is written to the
/// user's default project (auto-created on first activation if absent).
///
/// Status machine:
///   PENDING  →  ACTIVE   (activate / admin assign)
///   ACTIVE   →  OVERDUE  (payment overdue, swept by cron)
///   ACTIVE   →  EXPIRED  (expires_at reached, swept by cron or login)
///   ACTIVE   →  CANCELLED (user self-cancel or admin cancel)
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Extension, Json,
};
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::middleware::AuthUser,
    error::{AppError, AppResult},
    state::AppState,
};

fn require_admin(auth: &AuthUser) -> AppResult<()> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("global admin required".into()));
    }
    Ok(())
}

// ─── Quota helpers ────────────────────────────────────────────────────────────

/// Allocate a plan's quota to the user's default project.
/// The default project is auto-created if it doesn't exist yet.
async fn allocate_plan_to_default_project(
    state: &AppState,
    user_id: &str,
    plan_id: &str,
) -> AppResult<()> {
    // NB: managed-binding quotas (mq/smtp/redis/s3) live alongside the
    // existing DB/app quotas. They're loaded with non-macro query_as because
    // the new columns may not be in the sqlx prepare cache yet.
    #[derive(sqlx::FromRow)]
    struct PlanRow {
        quota_cpu_mcores: u32, quota_mem_mb: u32, quota_storage_gb: u32,
        quota_bandwidth_gb: u32, quota_domain_count: u32,
        quota_db_instance_count: u32, quota_app_count: u32, quota_request_million: u32,
        quota_mq_binding_count: u32, quota_smtp_binding_count: u32,
        quota_redis_binding_count: u32, quota_s3_binding_count: u32,
    }
    let plan: PlanRow = sqlx::query_as(
        "SELECT quota_cpu_mcores, quota_mem_mb, quota_storage_gb, \
                quota_bandwidth_gb, quota_domain_count, quota_db_instance_count, \
                quota_app_count, quota_request_million, \
                quota_mq_binding_count, quota_smtp_binding_count, \
                quota_redis_binding_count, quota_s3_binding_count \
         FROM subscription_plans WHERE id = ?",
    )
    .bind(plan_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("plan {plan_id}")))?;

    let project_id = find_or_create_default_project(state, user_id).await?;

    sqlx::query(
        "UPDATE projects SET \
             quota_cpu_mcores      = ?, \
             quota_mem_mb          = ?, \
             quota_storage_gb      = ?, \
             quota_bandwidth_gb    = ?, \
             quota_domain_count    = ?, \
             quota_db_instances    = ?, \
             quota_apps            = ?, \
             quota_request_million = ?, \
             quota_mq_bindings     = ?, \
             quota_smtp_bindings   = ?, \
             quota_redis_bindings  = ?, \
             quota_s3_bindings     = ? \
         WHERE id = ?",
    )
    .bind(plan.quota_cpu_mcores)
    .bind(plan.quota_mem_mb)
    .bind(plan.quota_storage_gb)
    .bind(plan.quota_bandwidth_gb)
    .bind(plan.quota_domain_count)
    .bind(plan.quota_db_instance_count)
    .bind(plan.quota_app_count)
    .bind(plan.quota_request_million)
    .bind(plan.quota_mq_binding_count)
    .bind(plan.quota_smtp_binding_count)
    .bind(plan.quota_redis_binding_count)
    .bind(plan.quota_s3_binding_count)
    .bind(&project_id)
    .execute(&state.db)
    .await?;

    Ok(())
}

/// Return the id of the user's default project, creating it if absent.
async fn find_or_create_default_project(state: &AppState, user_id: &str) -> AppResult<String> {
    if let Some(id) = sqlx::query_scalar!(
        r#"SELECT id FROM projects WHERE owner_id = ? AND is_default = 1 LIMIT 1"#,
        user_id
    )
    .fetch_optional(&state.db)
    .await?
    {
        return Ok(id);
    }

    let username = sqlx::query_scalar!(
        r#"SELECT username FROM users WHERE id = ?"#,
        user_id
    )
    .fetch_one(&state.db)
    .await?;

    let proj_id = Uuid::new_v4().to_string();

    // Build a valid k8s-namespace slug from the username
    let base: String = username
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let base = base.trim_matches('-').to_string();
    let base = if base.is_empty() { "user".to_string() } else { base };
    let candidate = format!("{}-default", &base[..base.len().min(54)]);

    // Resolve collision with a short UUID suffix
    let name_taken: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM projects WHERE name = ?"#,
        candidate
    )
    .fetch_one(&state.db)
    .await?;

    let proj_name = if name_taken == 0 {
        candidate
    } else {
        format!("{}-{}", &candidate[..candidate.len().min(54)], &proj_id[..8])
    };

    sqlx::query!(
        r#"INSERT INTO projects (id, name, display_name, owner_id, is_default)
           VALUES (?, ?, ?, ?, 1)"#,
        proj_id,
        proj_name,
        format!("{} 的默认项目", username),
        user_id,
    )
    .execute(&state.db)
    .await?;

    sqlx::query!(
        r#"INSERT INTO project_members (project_id, user_id, role)
           VALUES (?, ?, 'ADMIN')"#,
        proj_id,
        user_id,
    )
    .execute(&state.db)
    .await?;

    Ok(proj_id)
}

/// Zero-out all quota on the user's default project.
/// Called when subscription expires/cancels and the platform policy is RESET.
async fn reset_default_project_quotas(state: &AppState, user_id: &str) -> AppResult<()> {
    sqlx::query!(
        r#"UPDATE projects SET
             quota_cpu_mcores      = 0,
             quota_mem_mb          = 0,
             quota_storage_gb      = 0,
             quota_bandwidth_gb    = 0,
             quota_domain_count    = 0,
             quota_db_instances    = 0,
             quota_apps            = 0,
             quota_request_million = 0
           WHERE owner_id = ? AND is_default = 1"#,
        user_id
    )
    .execute(&state.db)
    .await?;
    Ok(())
}

/// Cancel the current ACTIVE (or OVERDUE) subscription for a user, if any.
async fn cancel_active(
    state: &AppState,
    user_id: &str,
    reason: &str,
) -> AppResult<Option<String>> {
    let row = sqlx::query_scalar!(
        r#"SELECT id FROM user_subscriptions
           WHERE user_id = ? AND status IN ('ACTIVE','OVERDUE')
           ORDER BY created_at DESC LIMIT 1"#,
        user_id
    )
    .fetch_optional(&state.db)
    .await?;

    let Some(sub_id) = row else {
        return Ok(None);
    };

    sqlx::query!(
        r#"UPDATE user_subscriptions
           SET status = 'CANCELLED', cancelled_at = NOW(), cancel_reason = ?
           WHERE id = ?"#,
        reason,
        sub_id,
    )
    .execute(&state.db)
    .await?;

    Ok(Some(sub_id))
}

// ─── Shared response shape ────────────────────────────────────────────────────

fn plan_json(r: &PlanRow) -> serde_json::Value {
    serde_json::json!({
        "id": r.id,
        "name": r.name,
        "display_name": r.display_name,
        "description": r.description,
        "price_monthly": r.price_monthly,
        "price_annually": r.price_annually,
        "quota": {
            "cpu_mcores":          r.quota_cpu_mcores,
            "mem_mb":              r.quota_mem_mb,
            "storage_gb":          r.quota_storage_gb,
            "bandwidth_gb":        r.quota_bandwidth_gb,
            "domain_count":        r.quota_domain_count,
            "db_instance_count":   r.quota_db_instance_count,
            "project_count":       r.quota_project_count,
            "app_count":           r.quota_app_count,
            "request_million":     r.quota_request_million,
            // P2c — managed-binding quotas (fetched separately via merge_p2c_quotas)
            "mq_binding_count":    0,
            "smtp_binding_count":  0,
            "redis_binding_count": 0,
            "s3_binding_count":    0,
        },
        "is_active": r.is_active != 0,
        "is_public": r.is_public != 0,
        "sort_order": r.sort_order,
        "created_at": r.created_at,
        "updated_at": r.updated_at,
    })
}

/// P2c — fetch the four managed-binding quotas for a plan and merge into
/// the response JSON produced by `plan_json`. Run after macro-based queries
/// so the older code keeps working.
async fn merge_p2c_quotas(state: &AppState, plan_id: &str, value: &mut serde_json::Value) {
    #[derive(sqlx::FromRow)]
    struct Q { mq: u32, smtp: u32, redis: u32, s3: u32 }
    if let Ok(Some(q)) = sqlx::query_as::<_, Q>(
        "SELECT quota_mq_binding_count AS mq, quota_smtp_binding_count AS smtp, \
                quota_redis_binding_count AS redis, quota_s3_binding_count AS s3 \
         FROM subscription_plans WHERE id = ?",
    )
    .bind(plan_id)
    .fetch_optional(&state.db)
    .await
    {
        if let Some(quota) = value.get_mut("quota") {
            if let Some(obj) = quota.as_object_mut() {
                obj.insert("mq_binding_count".into(),    q.mq.into());
                obj.insert("smtp_binding_count".into(),  q.smtp.into());
                obj.insert("redis_binding_count".into(), q.redis.into());
                obj.insert("s3_binding_count".into(),    q.s3.into());
            }
        }
    }
}

struct PlanRow {
    id: String,
    name: String,
    display_name: String,
    description: Option<String>,
    price_monthly: Decimal,
    price_annually: Option<Decimal>,
    quota_cpu_mcores: u32,
    quota_mem_mb: u32,
    quota_storage_gb: u32,
    quota_bandwidth_gb: u32,
    quota_domain_count: u32,
    quota_db_instance_count: u32,
    quota_project_count: u32,
    quota_app_count: u32,
    quota_request_million: u32,
    is_active: i8,
    is_public: i8,
    sort_order: i16,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

// ══════════════════════════════════════════════════════════════════════════════
// Public: plan catalogue (visible to all authenticated users)
// ══════════════════════════════════════════════════════════════════════════════

pub async fn list_plans(
    State(state): State<AppState>,
    Extension(_auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    let rows = sqlx::query!(
        r#"SELECT id, name, display_name, description,
                  price_monthly, price_annually,
                  quota_cpu_mcores, quota_mem_mb, quota_storage_gb,
                  quota_bandwidth_gb, quota_domain_count, quota_db_instance_count,
                  quota_project_count, quota_app_count, quota_request_million,
                  is_active, is_public, sort_order, created_at, updated_at
           FROM subscription_plans
           WHERE is_active = 1 AND is_public = 1
           ORDER BY sort_order, price_monthly"#
    )
    .fetch_all(&state.db)
    .await?;

    let mut plans: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            plan_json(&PlanRow {
                id: r.id, name: r.name, display_name: r.display_name,
                description: r.description,
                price_monthly: r.price_monthly, price_annually: r.price_annually,
                quota_cpu_mcores: r.quota_cpu_mcores, quota_mem_mb: r.quota_mem_mb,
                quota_storage_gb: r.quota_storage_gb, quota_bandwidth_gb: r.quota_bandwidth_gb,
                quota_domain_count: r.quota_domain_count,
                quota_db_instance_count: r.quota_db_instance_count,
                quota_project_count: r.quota_project_count, quota_app_count: r.quota_app_count,
                quota_request_million: r.quota_request_million,
                is_active: r.is_active, is_public: r.is_public,
                sort_order: r.sort_order, created_at: r.created_at, updated_at: r.updated_at,
            })
        })
        .collect();

    // P2c: enrich each row with managed-binding quotas
    for v in plans.iter_mut() {
        if let Some(id) = v.get("id").and_then(|x| x.as_str()).map(String::from) {
            merge_p2c_quotas(&state, &id, v).await;
        }
    }

    Ok(Json(plans))
}

pub async fn get_plan(
    State(state): State<AppState>,
    Extension(_auth): Extension<AuthUser>,
    Path(plan_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let r = sqlx::query!(
        r#"SELECT id, name, display_name, description,
                  price_monthly, price_annually,
                  quota_cpu_mcores, quota_mem_mb, quota_storage_gb,
                  quota_bandwidth_gb, quota_domain_count, quota_db_instance_count,
                  quota_project_count, quota_app_count, quota_request_million,
                  is_active, is_public, sort_order, created_at, updated_at
           FROM subscription_plans WHERE id = ? AND is_active = 1 AND is_public = 1"#,
        plan_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("plan {plan_id}")))?;

    let mut value = plan_json(&PlanRow {
        id: r.id.clone(), name: r.name, display_name: r.display_name, description: r.description,
        price_monthly: r.price_monthly, price_annually: r.price_annually,
        quota_cpu_mcores: r.quota_cpu_mcores, quota_mem_mb: r.quota_mem_mb,
        quota_storage_gb: r.quota_storage_gb, quota_bandwidth_gb: r.quota_bandwidth_gb,
        quota_domain_count: r.quota_domain_count,
        quota_db_instance_count: r.quota_db_instance_count,
        quota_project_count: r.quota_project_count, quota_app_count: r.quota_app_count,
        quota_request_million: r.quota_request_million,
        is_active: r.is_active, is_public: r.is_public,
        sort_order: r.sort_order, created_at: r.created_at, updated_at: r.updated_at,
    });
    merge_p2c_quotas(&state, &r.id, &mut value).await;
    Ok(Json(value))
}

// ══════════════════════════════════════════════════════════════════════════════
// User: self-service subscription
// ══════════════════════════════════════════════════════════════════════════════

pub async fn get_my_subscription(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    let row = sqlx::query!(
        r#"SELECT s.id, s.plan_id, s.status, s.billing_cycle,
                  s.started_at, s.expires_at, s.auto_renew,
                  s.cancelled_at, s.cancel_reason, s.price_paid, s.created_at,
                  p.display_name AS plan_display_name, p.name AS plan_name
           FROM user_subscriptions s
           JOIN subscription_plans p ON p.id = s.plan_id
           WHERE s.user_id = ?
           ORDER BY s.created_at DESC
           LIMIT 1"#,
        auth.user_id
    )
    .fetch_optional(&state.db)
    .await?;

    match row {
        None => Ok(Json(serde_json::json!({ "subscription": null }))),
        Some(s) => Ok(Json(serde_json::json!({ "subscription": {
            "id": s.id,
            "plan_id": s.plan_id,
            "plan_name": s.plan_name,
            "plan_display_name": s.plan_display_name,
            "status": s.status,
            "billing_cycle": s.billing_cycle,
            "started_at": s.started_at,
            "expires_at": s.expires_at,
            "auto_renew": s.auto_renew != 0,
            "cancelled_at": s.cancelled_at,
            "cancel_reason": s.cancel_reason,
            "price_paid": s.price_paid,
            "created_at": s.created_at,
        }}))),
    }
}

#[derive(Deserialize)]
pub struct SelfSubscribeRequest {
    pub plan_id: String,
    pub billing_cycle: Option<String>,
    pub auto_renew: Option<bool>,
}

pub async fn subscribe(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<SelfSubscribeRequest>,
) -> AppResult<impl IntoResponse> {
    let allowed = sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = 'subscription_self_service'"#
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_else(|| "1".to_string());
    if allowed != "1" {
        return Err(AppError::Forbidden("self-service subscription is disabled".into()));
    }

    let billing_cycle = body.billing_cycle.as_deref().unwrap_or("MONTHLY");
    if !matches!(billing_cycle, "MONTHLY" | "ANNUALLY") {
        return Err(AppError::BadRequest("billing_cycle must be MONTHLY or ANNUALLY".into()));
    }

    let plan = sqlx::query!(
        r#"SELECT id, display_name, price_monthly, price_annually, is_public
           FROM subscription_plans WHERE id = ? AND is_active = 1"#,
        body.plan_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("plan {}", body.plan_id)))?;

    if plan.is_public == 0 {
        return Err(AppError::Forbidden("this plan is not available for self-subscription".into()));
    }

    let (price, expires_at) = match billing_cycle {
        "ANNUALLY" => {
            let p = plan
                .price_annually
                .ok_or_else(|| AppError::BadRequest("this plan does not support annual billing".into()))?;
            (p, Some(Utc::now() + Duration::days(365)))
        }
        _ => (plan.price_monthly, Some(Utc::now() + Duration::days(30))),
    };

    if price < Decimal::ZERO {
        return Err(AppError::BadRequest("negative price".into()));
    }

    let allow_downgrade = sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = 'subscription_allow_downgrade'"#
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_else(|| "1".to_string());

    if allow_downgrade != "1" {
        let current_price = sqlx::query_scalar!(
            r#"SELECT p.price_monthly FROM user_subscriptions s
               JOIN subscription_plans p ON p.id = s.plan_id
               WHERE s.user_id = ? AND s.status IN ('ACTIVE','OVERDUE')
               ORDER BY s.created_at DESC LIMIT 1"#,
            auth.user_id
        )
        .fetch_optional(&state.db)
        .await?;

        if let Some(cp) = current_price {
            if price < cp {
                return Err(AppError::Forbidden("downgrading subscription is not allowed".into()));
            }
        }
    }

    // Deduct from wallet if billing is enabled and plan has a cost
    let billing_enabled = sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = 'billing_enabled'"#
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_else(|| "0".to_string());

    if billing_enabled == "1" && price > Decimal::ZERO {
        let balance = sqlx::query_scalar!(
            r#"SELECT balance FROM user_wallets WHERE user_id = ?"#,
            auth.user_id
        )
        .fetch_optional(&state.db)
        .await?
        .unwrap_or(Decimal::ZERO);

        if balance < price {
            return Err(AppError::BadRequest(format!(
                "insufficient balance: need {price}, have {balance}"
            )));
        }

        sqlx::query!(
            r#"UPDATE user_wallets SET balance = balance - ? WHERE user_id = ?"#,
            price,
            auth.user_id,
        )
        .execute(&state.db)
        .await?;

        let new_balance: Decimal = sqlx::query_scalar!(
            r#"SELECT balance FROM user_wallets WHERE user_id = ?"#,
            auth.user_id
        )
        .fetch_one(&state.db)
        .await?;

        sqlx::query!(
            r#"INSERT INTO wallet_transactions
               (id, user_id, tx_type, amount, balance_after, description, ref_id)
               VALUES (?, ?, 'DEDUCTION', ?, ?, ?, NULL)"#,
            Uuid::new_v4().to_string(),
            auth.user_id,
            -price,
            new_balance,
            format!("订阅 {}", plan.display_name),
        )
        .execute(&state.db)
        .await?;
    }

    cancel_active(&state, &auth.user_id, "replaced by new subscription").await?;

    let sub_id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO user_subscriptions
           (id, user_id, plan_id, status, billing_cycle,
            started_at, expires_at, auto_renew, price_paid)
           VALUES (?, ?, ?, 'ACTIVE', ?, NOW(), ?, ?, ?)"#,
        sub_id,
        auth.user_id,
        body.plan_id,
        billing_cycle,
        expires_at.map(|dt| dt.naive_utc()),
        body.auto_renew.unwrap_or(true) as i8,
        price,
    )
    .execute(&state.db)
    .await?;

    // Allocate plan quota to the user's default project
    allocate_plan_to_default_project(&state, &auth.user_id, &body.plan_id).await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({
            "subscription_id": sub_id,
            "status": "ACTIVE",
            "expires_at": expires_at,
        })),
    ))
}

#[derive(Deserialize)]
pub struct CancelSubscriptionRequest {
    pub reason: Option<String>,
}

pub async fn cancel_my_subscription(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CancelSubscriptionRequest>,
) -> AppResult<impl IntoResponse> {
    let reason = body.reason.as_deref().unwrap_or("user requested cancellation");
    let cancelled = cancel_active(&state, &auth.user_id, reason).await?;

    if cancelled.is_none() {
        return Err(AppError::NotFound("no active subscription to cancel".into()));
    }

    let action = sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = 'subscription_expiry_action'"#
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_else(|| "KEEP".to_string());

    if action == "RESET" {
        reset_default_project_quotas(&state, &auth.user_id).await?;
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ══════════════════════════════════════════════════════════════════════════════
// Admin: plan CRUD
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
pub struct AdminListPlansQuery {
    pub include_inactive: Option<bool>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

pub async fn admin_list_plans(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Query(q): Query<AdminListPlansQuery>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(50).min(200);
    let offset = (page - 1) * per_page;
    let include_inactive = q.include_inactive.unwrap_or(true);

    let rows = sqlx::query!(
        r#"SELECT p.id, p.name, p.display_name, p.description,
                  p.price_monthly, p.price_annually,
                  p.quota_cpu_mcores, p.quota_mem_mb, p.quota_storage_gb,
                  p.quota_bandwidth_gb, p.quota_domain_count, p.quota_db_instance_count,
                  p.quota_project_count, p.quota_app_count, p.quota_request_million,
                  p.is_active, p.is_public, p.sort_order,
                  p.created_at, p.updated_at,
                  (SELECT COUNT(*) FROM user_subscriptions
                   WHERE plan_id = p.id AND status IN ('ACTIVE','OVERDUE')) AS active_subscribers
           FROM subscription_plans p
           WHERE ? OR p.is_active = 1
           ORDER BY p.sort_order, p.price_monthly
           LIMIT ? OFFSET ?"#,
        include_inactive,
        per_page,
        offset,
    )
    .fetch_all(&state.db)
    .await?;

    let total: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM subscription_plans WHERE ? OR is_active = 1"#,
        include_inactive,
    )
    .fetch_one(&state.db)
    .await?;

    let rows_for_merge: Vec<String> = rows
        .iter()
        .filter_map(|r| r.id.clone())
        .collect();
    let mut data: Vec<serde_json::Value> = rows.into_iter().map(|r| {
        let mut v = plan_json(&PlanRow {
            id: r.id.unwrap_or_default(),
            name: r.name.unwrap_or_default(),
            display_name: r.display_name.unwrap_or_default(),
            description: r.description,
            price_monthly: r.price_monthly.unwrap_or_default(),
            price_annually: r.price_annually,
            quota_cpu_mcores: r.quota_cpu_mcores.unwrap_or(0) as u32,
            quota_mem_mb: r.quota_mem_mb.unwrap_or(0) as u32,
            quota_storage_gb: r.quota_storage_gb.unwrap_or(0) as u32,
            quota_bandwidth_gb: r.quota_bandwidth_gb.unwrap_or(0) as u32,
            quota_domain_count: r.quota_domain_count.unwrap_or(0) as u32,
            quota_db_instance_count: r.quota_db_instance_count.unwrap_or(0) as u32,
            quota_project_count: r.quota_project_count.unwrap_or(0) as u32,
            quota_app_count: r.quota_app_count.unwrap_or(0) as u32,
            quota_request_million: r.quota_request_million.unwrap_or(0) as u32,
            is_active: r.is_active.unwrap_or(1),
            is_public: r.is_public.unwrap_or(1),
            sort_order: r.sort_order.unwrap_or(0),
            created_at: r.created_at.unwrap_or_default(),
            updated_at: r.updated_at.unwrap_or_default(),
        });
        v["active_subscribers"] = serde_json::json!(r.active_subscribers);
        v
    }).collect();

    // P2c — merge managed-binding quotas
    for (v, id) in data.iter_mut().zip(rows_for_merge.iter()) {
        merge_p2c_quotas(&state, id, v).await;
    }

    Ok(Json(serde_json::json!({
        "data": data,
        "total": total,
        "page": page,
        "per_page": per_page,
    })))
}

pub async fn admin_get_plan(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(plan_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let r = sqlx::query!(
        r#"SELECT id, name, display_name, description,
                  price_monthly, price_annually,
                  quota_cpu_mcores, quota_mem_mb, quota_storage_gb,
                  quota_bandwidth_gb, quota_domain_count, quota_db_instance_count,
                  quota_project_count, quota_app_count, quota_request_million,
                  is_active, is_public, sort_order, created_at, updated_at
           FROM subscription_plans WHERE id = ?"#,
        plan_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("plan {plan_id}")))?;

    let active_count: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM user_subscriptions
           WHERE plan_id = ? AND status IN ('ACTIVE','OVERDUE')"#,
        plan_id
    )
    .fetch_one(&state.db)
    .await?;

    let mut v = plan_json(&PlanRow {
        id: r.id.clone(), name: r.name, display_name: r.display_name, description: r.description,
        price_monthly: r.price_monthly, price_annually: r.price_annually,
        quota_cpu_mcores: r.quota_cpu_mcores, quota_mem_mb: r.quota_mem_mb,
        quota_storage_gb: r.quota_storage_gb, quota_bandwidth_gb: r.quota_bandwidth_gb,
        quota_domain_count: r.quota_domain_count,
        quota_db_instance_count: r.quota_db_instance_count,
        quota_project_count: r.quota_project_count, quota_app_count: r.quota_app_count,
        quota_request_million: r.quota_request_million,
        is_active: r.is_active, is_public: r.is_public,
        sort_order: r.sort_order, created_at: r.created_at, updated_at: r.updated_at,
    });
    v["active_subscribers"] = serde_json::json!(active_count);
    merge_p2c_quotas(&state, &r.id, &mut v).await;

    Ok(Json(v))
}

#[derive(Deserialize)]
pub struct CreatePlanRequest {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub price_monthly: Decimal,
    pub price_annually: Option<Decimal>,
    pub quota_cpu_mcores: u32,
    pub quota_mem_mb: u32,
    pub quota_storage_gb: u32,
    pub quota_bandwidth_gb: u32,
    pub quota_domain_count: u32,
    pub quota_db_instance_count: u32,
    pub quota_project_count: u32,
    pub quota_app_count: u32,
    pub quota_request_million: u32,
    pub is_public: Option<bool>,
    pub sort_order: Option<i16>,
    // P2c — managed-binding quotas (0 = unlimited)
    #[serde(default)]
    pub quota_mq_binding_count: u32,
    #[serde(default)]
    pub quota_smtp_binding_count: u32,
    #[serde(default)]
    pub quota_redis_binding_count: u32,
    #[serde(default)]
    pub quota_s3_binding_count: u32,
}

pub async fn admin_create_plan(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<CreatePlanRequest>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    if body.name.is_empty() || body.display_name.is_empty() {
        return Err(AppError::BadRequest("name and display_name are required".into()));
    }
    if body.price_monthly < Decimal::ZERO {
        return Err(AppError::BadRequest("price cannot be negative".into()));
    }

    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO subscription_plans
           (id, name, display_name, description,
            price_monthly, price_annually,
            quota_cpu_mcores, quota_mem_mb, quota_storage_gb,
            quota_bandwidth_gb, quota_domain_count, quota_db_instance_count,
            quota_project_count, quota_app_count, quota_request_million,
            is_public, sort_order)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        id,
        body.name.trim(),
        body.display_name.trim(),
        body.description,
        body.price_monthly,
        body.price_annually,
        body.quota_cpu_mcores,
        body.quota_mem_mb,
        body.quota_storage_gb,
        body.quota_bandwidth_gb,
        body.quota_domain_count,
        body.quota_db_instance_count,
        body.quota_project_count,
        body.quota_app_count,
        body.quota_request_million,
        body.is_public.unwrap_or(true) as i8,
        body.sort_order.unwrap_or(0),
    )
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref de) if de.is_unique_violation() => {
            AppError::Conflict(format!("plan name '{}' already exists", body.name))
        }
        other => AppError::Database(other),
    })?;

    // P2c — apply managed-binding quotas via a non-macro UPDATE.
    // (Kept separate from the macro INSERT above to avoid prepare-cache misses.)
    sqlx::query(
        "UPDATE subscription_plans SET \
             quota_mq_binding_count    = ?, \
             quota_smtp_binding_count  = ?, \
             quota_redis_binding_count = ?, \
             quota_s3_binding_count    = ? \
         WHERE id = ?",
    )
    .bind(body.quota_mq_binding_count)
    .bind(body.quota_smtp_binding_count)
    .bind(body.quota_redis_binding_count)
    .bind(body.quota_s3_binding_count)
    .bind(&id)
    .execute(&state.db)
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "id": id })),
    ))
}

#[derive(Deserialize)]
pub struct UpdatePlanRequest {
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub price_monthly: Option<Decimal>,
    pub price_annually: Option<Decimal>,
    pub quota_cpu_mcores: Option<u32>,
    pub quota_mem_mb: Option<u32>,
    pub quota_storage_gb: Option<u32>,
    pub quota_bandwidth_gb: Option<u32>,
    pub quota_domain_count: Option<u32>,
    pub quota_db_instance_count: Option<u32>,
    pub quota_project_count: Option<u32>,
    pub quota_app_count: Option<u32>,
    pub quota_request_million: Option<u32>,
    pub is_active: Option<bool>,
    pub is_public: Option<bool>,
    pub sort_order: Option<i16>,
    // P2c — managed-binding quotas (None = leave unchanged)
    pub quota_mq_binding_count: Option<u32>,
    pub quota_smtp_binding_count: Option<u32>,
    pub quota_redis_binding_count: Option<u32>,
    pub quota_s3_binding_count: Option<u32>,
}

pub async fn admin_update_plan(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(plan_id): Path<String>,
    Json(body): Json<UpdatePlanRequest>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    sqlx::query!(
        r#"UPDATE subscription_plans SET
             display_name             = COALESCE(?, display_name),
             description              = COALESCE(?, description),
             price_monthly            = COALESCE(?, price_monthly),
             price_annually           = COALESCE(?, price_annually),
             quota_cpu_mcores         = COALESCE(?, quota_cpu_mcores),
             quota_mem_mb             = COALESCE(?, quota_mem_mb),
             quota_storage_gb         = COALESCE(?, quota_storage_gb),
             quota_bandwidth_gb       = COALESCE(?, quota_bandwidth_gb),
             quota_domain_count       = COALESCE(?, quota_domain_count),
             quota_db_instance_count  = COALESCE(?, quota_db_instance_count),
             quota_project_count      = COALESCE(?, quota_project_count),
             quota_app_count          = COALESCE(?, quota_app_count),
             quota_request_million    = COALESCE(?, quota_request_million),
             is_active                = COALESCE(?, is_active),
             is_public                = COALESCE(?, is_public),
             sort_order               = COALESCE(?, sort_order)
           WHERE id = ?"#,
        body.display_name,
        body.description,
        body.price_monthly,
        body.price_annually,
        body.quota_cpu_mcores,
        body.quota_mem_mb,
        body.quota_storage_gb,
        body.quota_bandwidth_gb,
        body.quota_domain_count,
        body.quota_db_instance_count,
        body.quota_project_count,
        body.quota_app_count,
        body.quota_request_million,
        body.is_active.map(|v| v as i8),
        body.is_public.map(|v| v as i8),
        body.sort_order,
        plan_id,
    )
    .execute(&state.db)
    .await?;

    // P2c — managed-binding quotas via non-macro UPDATE (None = leave unchanged)
    sqlx::query(
        "UPDATE subscription_plans SET \
             quota_mq_binding_count    = COALESCE(?, quota_mq_binding_count), \
             quota_smtp_binding_count  = COALESCE(?, quota_smtp_binding_count), \
             quota_redis_binding_count = COALESCE(?, quota_redis_binding_count), \
             quota_s3_binding_count    = COALESCE(?, quota_s3_binding_count) \
         WHERE id = ?",
    )
    .bind(body.quota_mq_binding_count)
    .bind(body.quota_smtp_binding_count)
    .bind(body.quota_redis_binding_count)
    .bind(body.quota_s3_binding_count)
    .bind(&plan_id)
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn admin_delete_plan(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(plan_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let active: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM user_subscriptions
           WHERE plan_id = ? AND status IN ('ACTIVE','OVERDUE')"#,
        plan_id
    )
    .fetch_one(&state.db)
    .await?;

    if active > 0 {
        return Err(AppError::Conflict(format!(
            "cannot delete plan with {active} active subscriber(s) — deactivate or migrate them first"
        )));
    }

    sqlx::query!(
        r#"DELETE FROM subscription_plans WHERE id = ?"#,
        plan_id
    )
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ══════════════════════════════════════════════════════════════════════════════
// Admin: subscription management
// ══════════════════════════════════════════════════════════════════════════════

#[derive(Deserialize)]
pub struct AdminListSubsQuery {
    pub user_id: Option<String>,
    pub plan_id: Option<String>,
    pub status: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

pub async fn admin_list_subscriptions(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Query(q): Query<AdminListSubsQuery>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(20).min(200);
    let offset = (page - 1) * per_page;

    let rows = sqlx::query!(
        r#"SELECT s.id, s.user_id, s.plan_id, s.status, s.billing_cycle,
                  s.started_at, s.expires_at, s.auto_renew,
                  s.cancelled_at, s.price_paid, s.created_at,
                  u.username, u.email,
                  p.display_name AS plan_display_name, p.name AS plan_name
           FROM user_subscriptions s
           JOIN users u ON u.id = s.user_id
           JOIN subscription_plans p ON p.id = s.plan_id
           WHERE (? IS NULL OR s.user_id = ?)
             AND (? IS NULL OR s.plan_id = ?)
             AND (? IS NULL OR s.status  = ?)
           ORDER BY s.created_at DESC
           LIMIT ? OFFSET ?"#,
        q.user_id.clone(), q.user_id.clone(),
        q.plan_id.clone(), q.plan_id.clone(),
        q.status.clone(),  q.status.clone(),
        per_page, offset,
    )
    .fetch_all(&state.db)
    .await?;

    let total: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM user_subscriptions s
           WHERE (? IS NULL OR s.user_id = ?)
             AND (? IS NULL OR s.plan_id = ?)
             AND (? IS NULL OR s.status  = ?)"#,
        q.user_id.clone(), q.user_id.clone(),
        q.plan_id.clone(), q.plan_id.clone(),
        q.status.clone(),  q.status.clone(),
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "data": rows.iter().map(|r| serde_json::json!({
            "id": r.id,
            "user_id": r.user_id,
            "username": r.username,
            "email": r.email,
            "plan_id": r.plan_id,
            "plan_name": r.plan_name,
            "plan_display_name": r.plan_display_name,
            "status": r.status,
            "billing_cycle": r.billing_cycle,
            "started_at": r.started_at,
            "expires_at": r.expires_at,
            "auto_renew": r.auto_renew != 0,
            "cancelled_at": r.cancelled_at,
            "price_paid": r.price_paid,
            "created_at": r.created_at,
        })).collect::<Vec<_>>(),
        "total": total,
        "page": page,
        "per_page": per_page,
    })))
}

/// GET /admin/users/:id/subscription
/// Returns the user's current subscription plus the actual quota allocation
/// on their default project (so the admin can see if overrides have been applied).
pub async fn admin_get_user_subscription(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(user_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let sub = sqlx::query!(
        r#"SELECT s.id, s.plan_id, s.status, s.billing_cycle,
                  s.started_at, s.expires_at, s.auto_renew,
                  s.cancelled_at, s.cancel_reason, s.price_paid, s.created_at,
                  p.name AS plan_name, p.display_name AS plan_display_name,
                  p.quota_cpu_mcores    AS plan_cpu,
                  p.quota_mem_mb        AS plan_mem,
                  p.quota_storage_gb    AS plan_storage,
                  p.quota_bandwidth_gb  AS plan_bandwidth,
                  p.quota_domain_count  AS plan_domains,
                  p.quota_db_instance_count AS plan_db_instances,
                  p.quota_app_count     AS plan_apps,
                  p.quota_request_million   AS plan_requests,
                  p.quota_project_count AS plan_projects
           FROM user_subscriptions s
           JOIN subscription_plans p ON p.id = s.plan_id
           WHERE s.user_id = ?
           ORDER BY s.created_at DESC
           LIMIT 1"#,
        user_id
    )
    .fetch_optional(&state.db)
    .await?;

    // Current allocation on the default project (may differ from plan if overridden)
    let alloc = sqlx::query!(
        r#"SELECT id, name, display_name,
                  quota_cpu_mcores, quota_mem_mb, quota_storage_gb,
                  quota_bandwidth_gb, quota_domain_count, quota_db_instances,
                  quota_apps, quota_request_million
           FROM projects WHERE owner_id = ? AND is_default = 1 LIMIT 1"#,
        user_id
    )
    .fetch_optional(&state.db)
    .await?;

    match sub {
        None => Ok(Json(serde_json::json!({
            "subscription": null,
            "default_project_allocation": alloc.map(|p| serde_json::json!({
                "project_id":   p.id,
                "project_name": p.name,
                "cpu_mcores":     p.quota_cpu_mcores,
                "mem_mb":         p.quota_mem_mb,
                "storage_gb":     p.quota_storage_gb,
                "bandwidth_gb":   p.quota_bandwidth_gb,
                "domain_count":   p.quota_domain_count,
                "db_instances":   p.quota_db_instances,
                "apps":           p.quota_apps,
                "request_million": p.quota_request_million,
            })),
        }))),
        Some(s) => Ok(Json(serde_json::json!({
            "subscription": {
                "id": s.id,
                "plan_id": s.plan_id,
                "plan_name": s.plan_name,
                "plan_display_name": s.plan_display_name,
                "status": s.status,
                "billing_cycle": s.billing_cycle,
                "started_at": s.started_at,
                "expires_at": s.expires_at,
                "auto_renew": s.auto_renew != 0,
                "cancelled_at": s.cancelled_at,
                "cancel_reason": s.cancel_reason,
                "price_paid": s.price_paid,
                "created_at": s.created_at,
                // Plan's original quota — for comparison with actual allocation below
                "plan_quota": {
                    "cpu_mcores":     s.plan_cpu,
                    "mem_mb":         s.plan_mem,
                    "storage_gb":     s.plan_storage,
                    "bandwidth_gb":   s.plan_bandwidth,
                    "domain_count":   s.plan_domains,
                    "db_instances":   s.plan_db_instances,
                    "apps":           s.plan_apps,
                    "request_million": s.plan_requests,
                    "project_count":  s.plan_projects,
                },
            },
            // Actual quota currently on the default project (may have admin overrides)
            "default_project_allocation": alloc.map(|p| serde_json::json!({
                "project_id":    p.id,
                "project_name":  p.name,
                "display_name":  p.display_name,
                "cpu_mcores":     p.quota_cpu_mcores,
                "mem_mb":         p.quota_mem_mb,
                "storage_gb":     p.quota_storage_gb,
                "bandwidth_gb":   p.quota_bandwidth_gb,
                "domain_count":   p.quota_domain_count,
                "db_instances":   p.quota_db_instances,
                "apps":           p.quota_apps,
                "request_million": p.quota_request_million,
            })),
        }))),
    }
}

/// POST /admin/users/:id/subscription
///
/// Two modes in one endpoint:
///
/// 1. **Assign a new plan** (`plan_id` required):
///    - Cancels any current subscription (no billing deduction).
///    - Creates a new ACTIVE subscription.
///    - Writes plan quota to the default project (unless `skip_quota_apply`).
///    - Individual quota override fields are applied on top afterwards.
///
/// 2. **Patch quota only** (`plan_id` absent):
///    - The current subscription record is not touched.
///    - Only the provided quota override fields are written to the default project.
///    - Requires an existing active subscription (so the default project exists).
#[derive(Deserialize)]
pub struct AdminAssignRequest {
    pub plan_id: Option<String>,
    pub billing_cycle: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub auto_renew: Option<bool>,
    /// Skip writing plan quota to the default project (plan-assign mode only).
    pub skip_quota_apply: Option<bool>,
    // Per-dimension quota overrides — applied to the default project after plan
    // quota (if any). Omit a field to leave that dimension unchanged.
    pub quota_cpu_mcores: Option<u32>,
    pub quota_mem_mb: Option<u32>,
    pub quota_storage_gb: Option<u32>,
    pub quota_bandwidth_gb: Option<u32>,
    pub quota_domain_count: Option<u32>,
    pub quota_db_instances: Option<u32>,
    pub quota_apps: Option<u32>,
    pub quota_request_million: Option<u32>,
}

fn has_quota_overrides(body: &AdminAssignRequest) -> bool {
    body.quota_cpu_mcores.is_some()
        || body.quota_mem_mb.is_some()
        || body.quota_storage_gb.is_some()
        || body.quota_bandwidth_gb.is_some()
        || body.quota_domain_count.is_some()
        || body.quota_db_instances.is_some()
        || body.quota_apps.is_some()
        || body.quota_request_million.is_some()
}

pub async fn admin_assign_plan(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(user_id): Path<String>,
    Json(body): Json<AdminAssignRequest>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    // ── Mode 1: assign a new plan ─────────────────────────────────────────────
    if let Some(ref plan_id) = body.plan_id {
        let billing_cycle = body.billing_cycle.as_deref().unwrap_or("CUSTOM");

        let (price_monthly, price_annually): (Decimal, Option<Decimal>) = sqlx::query_as(
            "SELECT price_monthly, price_annually FROM subscription_plans \
             WHERE id = ? AND is_active = 1",
        )
        .bind(plan_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("plan {plan_id}")))?;

        // Revenue attribution: a comp/gift assignment is not charged to the user's
        // wallet, but we still record the plan's list value for the chosen cycle as
        // `price_paid` so comps show up in revenue/MRR reporting instead of as 0.
        // (price_paid is a recorded attribute — it never triggers a charge.)
        let price_paid: Decimal = match billing_cycle {
            "ANNUALLY" => price_annually.unwrap_or(price_monthly * Decimal::from(12)),
            // MONTHLY / CUSTOM / LIFETIME → attribute the monthly list value as baseline
            _ => price_monthly,
        };

        cancel_active(&state, &user_id, "replaced by admin assignment").await?;

        let sub_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO user_subscriptions \
               (id, user_id, plan_id, status, billing_cycle, \
                started_at, expires_at, auto_renew, price_paid, created_by) \
             VALUES (?, ?, ?, 'ACTIVE', ?, NOW(), ?, ?, ?, ?)",
        )
        .bind(&sub_id)
        .bind(&user_id)
        .bind(plan_id)
        .bind(billing_cycle)
        .bind(body.expires_at.map(|dt| dt.naive_utc()))
        .bind(body.auto_renew.unwrap_or(false) as i8)
        .bind(price_paid)
        .bind(&auth.user_id)
        .execute(&state.db)
        .await?;

        // Write full plan quota to default project (creates it if absent)
        if !body.skip_quota_apply.unwrap_or(false) {
            allocate_plan_to_default_project(&state, &user_id, plan_id).await?;
        }

        // Apply per-dimension overrides on top if any were provided
        if has_quota_overrides(&body) {
            apply_quota_overrides(&state, &user_id, &body).await?;
        }

        return Ok((
            axum::http::StatusCode::CREATED,
            Json(serde_json::json!({
                "mode": "plan_assigned",
                "subscription_id": sub_id,
                "status": "ACTIVE",
                "price_paid": price_paid,
            })),
        ));
    }

    // ── Mode 2: patch quota only (no plan change) ─────────────────────────────
    if !has_quota_overrides(&body) {
        return Err(AppError::BadRequest(
            "provide plan_id to assign a plan, or quota fields to patch the current allocation".into(),
        ));
    }

    // Verify user has an active subscription (and thus a default project)
    let has_active = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM user_subscriptions
           WHERE user_id = ? AND status IN ('ACTIVE','OVERDUE')"#,
        user_id
    )
    .fetch_one(&state.db)
    .await?;

    if has_active == 0 {
        return Err(AppError::BadRequest(
            "user has no active subscription; assign a plan first".into(),
        ));
    }

    apply_quota_overrides(&state, &user_id, &body).await?;

    Ok((
        axum::http::StatusCode::OK,
        Json(serde_json::json!({ "mode": "quota_patched" })),
    ))
}

/// Write only the provided quota fields to the user's default project.
/// Fields that are None are left unchanged (COALESCE semantics).
async fn apply_quota_overrides(
    state: &AppState,
    user_id: &str,
    body: &AdminAssignRequest,
) -> AppResult<()> {
    // Ensure default project exists before trying to patch it
    find_or_create_default_project(state, user_id).await?;

    sqlx::query!(
        r#"UPDATE projects SET
             quota_cpu_mcores      = COALESCE(?, quota_cpu_mcores),
             quota_mem_mb          = COALESCE(?, quota_mem_mb),
             quota_storage_gb      = COALESCE(?, quota_storage_gb),
             quota_bandwidth_gb    = COALESCE(?, quota_bandwidth_gb),
             quota_domain_count    = COALESCE(?, quota_domain_count),
             quota_db_instances    = COALESCE(?, quota_db_instances),
             quota_apps            = COALESCE(?, quota_apps),
             quota_request_million = COALESCE(?, quota_request_million)
           WHERE owner_id = ? AND is_default = 1"#,
        body.quota_cpu_mcores,
        body.quota_mem_mb,
        body.quota_storage_gb,
        body.quota_bandwidth_gb,
        body.quota_domain_count,
        body.quota_db_instances,
        body.quota_apps,
        body.quota_request_million,
        user_id,
    )
    .execute(&state.db)
    .await?;

    Ok(())
}

#[derive(Deserialize)]
pub struct AdminUpdateSubRequest {
    pub expires_at: Option<DateTime<Utc>>,
    pub auto_renew: Option<bool>,
    pub status: Option<String>,
    pub cancel_reason: Option<String>,
}

pub async fn admin_update_subscription(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(sub_id): Path<String>,
    Json(body): Json<AdminUpdateSubRequest>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    if let Some(ref s) = body.status {
        if !matches!(s.as_str(), "PENDING" | "ACTIVE" | "OVERDUE" | "EXPIRED" | "CANCELLED") {
            return Err(AppError::BadRequest(format!("invalid status '{s}'")));
        }
    }

    sqlx::query!(
        r#"UPDATE user_subscriptions SET
             expires_at    = COALESCE(?, expires_at),
             auto_renew    = COALESCE(?, auto_renew),
             status        = COALESCE(?, status),
             cancel_reason = COALESCE(?, cancel_reason),
             cancelled_at  = CASE WHEN ? = 'CANCELLED' AND cancelled_at IS NULL
                                  THEN NOW() ELSE cancelled_at END
           WHERE id = ?"#,
        body.expires_at.map(|dt| dt.naive_utc()),
        body.auto_renew.map(|v| v as i8),
        body.status,
        body.cancel_reason,
        body.status,
        sub_id,
    )
    .execute(&state.db)
    .await?;

    // Re-allocate quota to default project when reactivating
    if body.status.as_deref() == Some("ACTIVE") {
        let row = sqlx::query!(
            r#"SELECT plan_id, user_id FROM user_subscriptions WHERE id = ?"#,
            sub_id
        )
        .fetch_optional(&state.db)
        .await?;

        if let Some(r) = row {
            allocate_plan_to_default_project(&state, &r.user_id, &r.plan_id).await?;
        }
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct AdminCancelSubRequest {
    pub reason: Option<String>,
    pub reset_quotas: Option<bool>,
}

pub async fn admin_cancel_subscription(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(sub_id): Path<String>,
    Json(body): Json<AdminCancelSubRequest>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let row = sqlx::query!(
        r#"SELECT user_id, status FROM user_subscriptions WHERE id = ?"#,
        sub_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("subscription {sub_id}")))?;

    if row.status == "CANCELLED" {
        return Err(AppError::BadRequest("subscription already cancelled".into()));
    }

    sqlx::query!(
        r#"UPDATE user_subscriptions
           SET status = 'CANCELLED', cancelled_at = NOW(), cancel_reason = ?
           WHERE id = ?"#,
        body.reason.as_deref().unwrap_or("admin cancelled"),
        sub_id,
    )
    .execute(&state.db)
    .await?;

    if body.reset_quotas.unwrap_or(false) {
        reset_default_project_quotas(&state, &row.user_id).await?;
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}
