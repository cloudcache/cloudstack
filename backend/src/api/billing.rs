/// Billing API — wallet, transactions, usage snapshots, invoices.
///
/// User-facing routes operate on the calling user.
/// Admin routes are prefixed with `/admin/billing/`.
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
    Extension, Json,
};
use chrono::NaiveDate;
use serde::Deserialize;
use sqlx::Row;
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

// ─── Wallet ───────────────────────────────────────────────────────────────────

pub async fn get_wallet(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    let wallet = sqlx::query!(
        r#"SELECT balance, currency, updated_at FROM user_wallets WHERE user_id = ?"#,
        auth.user_id
    )
    .fetch_optional(&state.db)
    .await?;

    match wallet {
        Some(w) => Ok(Json(serde_json::json!({
            "balance": w.balance,
            "currency": w.currency,
            "updated_at": w.updated_at,
        }))),
        None => {
            let currency = crate::billing::billing_currency(&state).await;
            Ok(Json(serde_json::json!({
                "balance": 0,
                "currency": currency,
                "updated_at": null,
            })))
        }
    }
}

// ─── Transactions ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TransactionQuery {
    pub tx_type: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

pub async fn list_transactions(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Query(q): Query<TransactionQuery>,
) -> AppResult<impl IntoResponse> {
    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let rows = sqlx::query!(
        r#"SELECT id, tx_type, amount, balance_after, description, ref_id, created_at
           FROM wallet_transactions
           WHERE user_id = ?
             AND (? IS NULL OR tx_type = ?)
           ORDER BY created_at DESC
           LIMIT ? OFFSET ?"#,
        auth.user_id,
        q.tx_type.clone(),
        q.tx_type.clone(),
        per_page,
        offset,
    )
    .fetch_all(&state.db)
    .await?;

    let total: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM wallet_transactions
           WHERE user_id = ? AND (? IS NULL OR tx_type = ?)"#,
        auth.user_id,
        q.tx_type.clone(),
        q.tx_type.clone(),
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "data": rows.iter().map(|r| serde_json::json!({
            "id": r.id,
            "type": r.tx_type,
            "amount": r.amount,
            "balance_after": r.balance_after,
            "description": r.description,
            "ref_id": r.ref_id,
            "created_at": r.created_at,
        })).collect::<Vec<_>>(),
        "total": total,
        "page": page,
        "per_page": per_page,
    })))
}

// ─── Usage ────────────────────────────────────────────────────────────────────

pub async fn get_current_usage(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    // Live counts
    let active_apps: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM apps a
           WHERE a.owner_id = ? AND a.status NOT IN ('STOPPED','PAUSED')"#,
        auth.user_id
    )
    .fetch_one(&state.db)
    .await?;

    let active_dbs: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM database_instances WHERE created_by = ? AND status = 'ACTIVE'"#,
        auth.user_id
    )
    .fetch_one(&state.db)
    .await?;

    // Last hourly snapshot
    let last = sqlx::query!(
        r#"SELECT cpu_mcores_used, mem_mb_used, storage_gb_used, cost, snapshot_time
           FROM usage_snapshots WHERE user_id = ? ORDER BY snapshot_time DESC LIMIT 1"#,
        auth.user_id
    )
    .fetch_optional(&state.db)
    .await?;

    // Month-to-date cost
    let mtd_cost = sqlx::query_scalar!(
        r#"SELECT COALESCE(SUM(cost), 0) FROM usage_snapshots
           WHERE user_id = ? AND snapshot_time >= DATE_FORMAT(NOW(), '%Y-%m-01')"#,
        auth.user_id
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "active_apps": active_apps,
        "active_databases": active_dbs,
        "mtd_cost": mtd_cost,
        "last_snapshot": last.map(|s| serde_json::json!({
            "time": s.snapshot_time,
            "cpu_mcores": s.cpu_mcores_used,
            "mem_mb": s.mem_mb_used,
            "storage_gb": s.storage_gb_used,
            "hourly_cost": s.cost,
        })),
    })))
}

#[derive(Deserialize)]
pub struct UsageHistoryQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

pub async fn list_usage_history(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Query(q): Query<UsageHistoryQuery>,
) -> AppResult<impl IntoResponse> {
    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(48).min(744); // up to 31 days × 24h

    let offset = (page - 1) * per_page;

    let rows = sqlx::query!(
        r#"SELECT id, snapshot_time, app_count, db_count,
                  cpu_mcores_used, mem_mb_used, storage_gb_used, cost
           FROM usage_snapshots
           WHERE user_id = ?
             AND (? IS NULL OR snapshot_time >= ?)
             AND (? IS NULL OR snapshot_time <= ?)
           ORDER BY snapshot_time DESC
           LIMIT ? OFFSET ?"#,
        auth.user_id,
        q.from,
        q.from,
        q.to,
        q.to,
        per_page,
        offset,
    )
    .fetch_all(&state.db)
    .await?;

    let total: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM usage_snapshots
           WHERE user_id = ?
             AND (? IS NULL OR snapshot_time >= ?)
             AND (? IS NULL OR snapshot_time <= ?)"#,
        auth.user_id,
        q.from,
        q.from,
        q.to,
        q.to,
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "data": rows.iter().map(|r| serde_json::json!({
            "id": r.id,
            "time": r.snapshot_time,
            "apps": r.app_count,
            "databases": r.db_count,
            "cpu_mcores": r.cpu_mcores_used,
            "mem_mb": r.mem_mb_used,
            "storage_gb": r.storage_gb_used,
            "cost": r.cost,
        })).collect::<Vec<_>>(),
        "total": total,
        "page": page,
        "per_page": per_page,
    })))
}

// ─── Invoices ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct InvoiceQuery {
    pub status: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

pub async fn list_invoices(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Query(q): Query<InvoiceQuery>,
) -> AppResult<impl IntoResponse> {
    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(12).min(60);
    let offset = (page - 1) * per_page;

    let rows = sqlx::query!(
        r#"SELECT id, invoice_no, period_start, period_end, total_amount, status, created_at, issued_at, paid_at
           FROM invoices
           WHERE user_id = ?
             AND (? IS NULL OR status = ?)
           ORDER BY period_start DESC
           LIMIT ? OFFSET ?"#,
        auth.user_id,
        q.status.clone(),
        q.status.clone(),
        per_page,
        offset,
    )
    .fetch_all(&state.db)
    .await?;

    let total: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM invoices
           WHERE user_id = ? AND (? IS NULL OR status = ?)"#,
        auth.user_id,
        q.status.clone(),
        q.status.clone(),
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "data": rows.iter().map(|r| serde_json::json!({
            "id": r.id,
            "invoice_no": r.invoice_no,
            "period_start": r.period_start,
            "period_end": r.period_end,
            "total_amount": r.total_amount,
            "status": r.status,
            "created_at": r.created_at,
            "issued_at": r.issued_at,
            "paid_at": r.paid_at,
        })).collect::<Vec<_>>(),
        "total": total,
        "page": page,
        "per_page": per_page,
    })))
}

pub async fn get_invoice(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(invoice_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let row = sqlx::query!(
        r#"SELECT id, invoice_no, period_start, period_end, total_amount,
                  status, items, created_at, issued_at, paid_at
           FROM invoices WHERE id = ? AND user_id = ?"#,
        invoice_id,
        auth.user_id,
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("invoice {invoice_id}")))?;

    Ok(Json(serde_json::json!({
        "id": row.id,
        "invoice_no": row.invoice_no,
        "period_start": row.period_start,
        "period_end": row.period_end,
        "total_amount": row.total_amount,
        "status": row.status,
        "items": row.items,
        "created_at": row.created_at,
        "issued_at": row.issued_at,
        "paid_at": row.paid_at,
    })))
}

// ─── Admin: wallets overview ──────────────────────────────────────────────────

pub async fn admin_list_wallets(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Query(q): Query<super::users::ListQuery>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(20).min(200);
    let offset = (page - 1) * per_page;

    let rows = sqlx::query!(
        r#"SELECT u.id, u.username, u.email, u.is_active,
                  COALESCE(w.balance, 0) AS balance,
                  COALESCE(w.currency, 'CNY') AS currency,
                  w.updated_at
           FROM users u
           LEFT JOIN user_wallets w ON w.user_id = u.id
           WHERE ? IS NULL OR u.username LIKE ? OR u.email LIKE ?
           ORDER BY u.username
           LIMIT ? OFFSET ?"#,
        q.search.as_ref().map(|s| format!("%{s}%")),
        q.search.as_ref().map(|s| format!("%{s}%")),
        q.search.as_ref().map(|s| format!("%{s}%")),
        per_page,
        offset,
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(serde_json::json!(rows
        .iter()
        .map(|r| serde_json::json!({
            "user_id": r.id,
            "username": r.username,
            "email": r.email,
            "is_active": r.is_active != 0,
            "balance": r.balance,
            "currency": r.currency,
            "updated_at": r.updated_at,
        }))
        .collect::<Vec<_>>())))
}

// ─── Admin: transaction ledger ────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct AdminTransactionQuery {
    pub search: Option<String>,
    pub tx_type: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

pub async fn admin_list_transactions(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Query(q): Query<AdminTransactionQuery>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(50).min(200);
    let offset = (page - 1) * per_page;
    let search = q.search.as_ref().map(|s| format!("%{s}%"));

    let rows = sqlx::query(
        r#"SELECT wt.id, wt.user_id, u.username, u.email, wt.tx_type, wt.amount,
                  wt.balance_after, wt.description, wt.ref_id, wt.operator_id,
                  op.username AS operator_username, wt.created_at
           FROM wallet_transactions wt
           JOIN users u ON u.id = wt.user_id
           LEFT JOIN users op ON op.id = wt.operator_id
           WHERE (? IS NULL OR u.username LIKE ? OR u.email LIKE ?)
             AND (? IS NULL OR wt.tx_type = ?)
           ORDER BY wt.created_at DESC
           LIMIT ? OFFSET ?"#,
    )
    .bind(search.clone())
    .bind(search.clone())
    .bind(search.clone())
    .bind(q.tx_type.clone())
    .bind(q.tx_type.clone())
    .bind(per_page)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;

    let total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM wallet_transactions wt
           JOIN users u ON u.id = wt.user_id
           WHERE (? IS NULL OR u.username LIKE ? OR u.email LIKE ?)
             AND (? IS NULL OR wt.tx_type = ?)"#,
    )
    .bind(search.clone())
    .bind(search.clone())
    .bind(search)
    .bind(q.tx_type.clone())
    .bind(q.tx_type)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "data": rows.iter().map(|r| serde_json::json!({
            "id": r.get::<String, _>("id"),
            "user_id": r.get::<String, _>("user_id"),
            "username": r.get::<String, _>("username"),
            "email": r.try_get::<String, _>("email").ok(),
            "type": r.get::<String, _>("tx_type"),
            "amount": r.get::<rust_decimal::Decimal, _>("amount"),
            "balance_after": r.get::<rust_decimal::Decimal, _>("balance_after"),
            "description": r.try_get::<String, _>("description").ok(),
            "ref_id": r.try_get::<String, _>("ref_id").ok(),
            "operator_id": r.try_get::<String, _>("operator_id").ok(),
            "operator_username": r.try_get::<String, _>("operator_username").ok(),
            "created_at": r.get::<chrono::NaiveDateTime, _>("created_at"),
        })).collect::<Vec<_>>(),
        "total": total,
        "page": page,
        "per_page": per_page,
    })))
}

// ─── Admin: recharge ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RechargeRequest {
    pub user_id: String,
    pub amount: rust_decimal::Decimal,
    pub description: Option<String>,
}

pub async fn admin_recharge(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<RechargeRequest>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    if body.amount <= rust_decimal::Decimal::ZERO {
        return Err(AppError::BadRequest(
            "recharge amount must be positive".into(),
        ));
    }

    let mut db_tx = state.db.begin().await?;

    sqlx::query!(
        r#"INSERT INTO user_wallets (user_id, balance) VALUES (?, ?)
           ON DUPLICATE KEY UPDATE balance = balance + VALUES(balance)"#,
        body.user_id,
        body.amount,
    )
    .execute(&mut *db_tx)
    .await?;

    let new_balance = sqlx::query_scalar!(
        r#"SELECT balance FROM user_wallets WHERE user_id = ?"#,
        body.user_id
    )
    .fetch_one(&mut *db_tx)
    .await?;

    let tx_id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO wallet_transactions
           (id, user_id, tx_type, amount, balance_after, description, operator_id)
           VALUES (?, ?, 'RECHARGE', ?, ?, ?, ?)"#,
        tx_id,
        body.user_id,
        body.amount,
        new_balance,
        body.description,
        auth.user_id,
    )
    .execute(&mut *db_tx)
    .await?;

    db_tx.commit().await?;

    Ok(Json(serde_json::json!({
        "transaction_id": tx_id,
        "new_balance": new_balance,
    })))
}

// ─── Admin: manual balance adjustment ────────────────────────────────────────

#[derive(Deserialize)]
pub struct AdjustmentRequest {
    pub user_id: String,
    /// Positive or negative delta
    pub amount: rust_decimal::Decimal,
    pub description: String,
}

pub async fn admin_adjust_balance(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<AdjustmentRequest>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    if body.description.trim().is_empty() {
        return Err(AppError::BadRequest(
            "description is required for adjustments".into(),
        ));
    }

    let mut db_tx = state.db.begin().await?;

    sqlx::query!(
        r#"INSERT INTO user_wallets (user_id, balance) VALUES (?, ?)
           ON DUPLICATE KEY UPDATE balance = balance + VALUES(balance)"#,
        body.user_id,
        body.amount,
    )
    .execute(&mut *db_tx)
    .await?;

    let new_balance = sqlx::query_scalar!(
        r#"SELECT balance FROM user_wallets WHERE user_id = ?"#,
        body.user_id
    )
    .fetch_one(&mut *db_tx)
    .await?;

    let tx_id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO wallet_transactions
           (id, user_id, tx_type, amount, balance_after, description, operator_id)
           VALUES (?, ?, 'ADJUSTMENT', ?, ?, ?, ?)"#,
        tx_id,
        body.user_id,
        body.amount,
        new_balance,
        body.description.trim(),
        auth.user_id,
    )
    .execute(&mut *db_tx)
    .await?;

    db_tx.commit().await?;

    Ok(Json(serde_json::json!({
        "transaction_id": tx_id,
        "new_balance": new_balance,
    })))
}

// ─── Admin: generate monthly invoice ─────────────────────────────────────────

#[derive(Deserialize)]
pub struct GenerateInvoiceRequest {
    pub user_id: String,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
}

pub async fn admin_generate_invoice(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<GenerateInvoiceRequest>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    if body.period_end <= body.period_start {
        return Err(AppError::BadRequest(
            "period_end must be after period_start".into(),
        ));
    }

    // Aggregate snapshots in the period
    let snapshots = sqlx::query!(
        r#"SELECT SUM(cost) AS total_cost,
                  SUM(cpu_mcores_used) AS total_cpu,
                  SUM(mem_mb_used) AS total_mem,
                  COUNT(*) AS hours
           FROM usage_snapshots
           WHERE user_id = ?
             AND snapshot_time >= ? AND snapshot_time < ?"#,
        body.user_id,
        body.period_start,
        body.period_end,
    )
    .fetch_one(&state.db)
    .await?;

    let total = snapshots.total_cost.unwrap_or_default();

    // Build line items JSON
    let items = serde_json::json!([{
        "description": format!("计算资源 ({} 小时)", snapshots.hours),
        "amount": total,
    }]);

    let invoice_no = generate_invoice_no();
    let id = Uuid::new_v4().to_string();

    sqlx::query!(
        r#"INSERT INTO invoices
           (id, user_id, invoice_no, period_start, period_end, total_amount, status, items, issued_at)
           VALUES (?, ?, ?, ?, ?, ?, 'ISSUED', ?, NOW())"#,
        id,
        body.user_id,
        invoice_no,
        body.period_start,
        body.period_end,
        total,
        items.to_string(),
    )
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref de) if de.is_unique_violation() => {
            AppError::Conflict(format!("invoice for this period already exists"))
        }
        other => AppError::Database(other),
    })?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({
            "id": id,
            "invoice_no": invoice_no,
            "total_amount": total,
        })),
    ))
}

// ─── Admin: list all invoices ─────────────────────────────────────────────────

pub async fn admin_list_invoices(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Query(q): Query<InvoiceQuery>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(20).min(200);
    let offset = (page - 1) * per_page;

    let rows = sqlx::query!(
        r#"SELECT i.id, i.invoice_no, u.username, i.period_start, i.period_end,
                  i.total_amount, i.status, i.created_at, i.issued_at, i.paid_at
           FROM invoices i
           JOIN users u ON u.id = i.user_id
           WHERE ? IS NULL OR i.status = ?
           ORDER BY i.created_at DESC
           LIMIT ? OFFSET ?"#,
        q.status.clone(),
        q.status.clone(),
        per_page,
        offset,
    )
    .fetch_all(&state.db)
    .await?;

    let total: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM invoices WHERE ? IS NULL OR status = ?"#,
        q.status.clone(),
        q.status.clone(),
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "data": rows.iter().map(|r| serde_json::json!({
            "id": r.id,
            "invoice_no": r.invoice_no,
            "username": r.username,
            "period_start": r.period_start,
            "period_end": r.period_end,
            "total_amount": r.total_amount,
            "status": r.status,
            "created_at": r.created_at,
            "issued_at": r.issued_at,
            "paid_at": r.paid_at,
        })).collect::<Vec<_>>(),
        "total": total,
        "page": page,
        "per_page": per_page,
    })))
}

// ─── Admin: mark invoice paid ─────────────────────────────────────────────────

pub async fn admin_mark_paid(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(invoice_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let row = sqlx::query!(
        r#"SELECT id, user_id, total_amount, status FROM invoices WHERE id = ?"#,
        invoice_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("invoice {invoice_id}")))?;

    if row.status == "PAID" {
        return Err(AppError::BadRequest("invoice already paid".into()));
    }
    if row.status == "VOID" {
        return Err(AppError::BadRequest("cannot pay a voided invoice".into()));
    }

    // Deduct from wallet
    sqlx::query!(
        r#"UPDATE user_wallets SET balance = balance - ? WHERE user_id = ?"#,
        row.total_amount,
        row.user_id,
    )
    .execute(&state.db)
    .await?;

    let new_balance = sqlx::query_scalar!(
        r#"SELECT balance FROM user_wallets WHERE user_id = ?"#,
        row.user_id
    )
    .fetch_one(&state.db)
    .await?;

    // Record deduction transaction
    sqlx::query!(
        r#"INSERT INTO wallet_transactions
           (id, user_id, tx_type, amount, balance_after, description, ref_id, operator_id)
           VALUES (?, ?, 'DEDUCTION', ?, ?, '账单扣款', ?, ?)"#,
        Uuid::new_v4().to_string(),
        row.user_id,
        -row.total_amount,
        new_balance,
        invoice_id,
        auth.user_id,
    )
    .execute(&state.db)
    .await?;

    sqlx::query!(
        r#"UPDATE invoices SET status = 'PAID', paid_at = NOW() WHERE id = ?"#,
        invoice_id
    )
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Admin: void invoice ──────────────────────────────────────────────────────

pub async fn admin_void_invoice(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(invoice_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    require_admin(&auth)?;

    let row = sqlx::query(r#"SELECT status FROM invoices WHERE id = ?"#)
        .bind(&invoice_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("invoice {invoice_id}")))?;

    if row.get::<String, _>("status") == "PAID" {
        return Err(AppError::BadRequest("cannot void a paid invoice".into()));
    }

    sqlx::query(r#"UPDATE invoices SET status = 'VOID' WHERE id = ?"#)
        .bind(invoice_id)
        .execute(&state.db)
        .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

fn generate_invoice_no() -> String {
    use chrono::Utc;
    let now = Utc::now();
    format!("INV-{}-{:06}", now.format("%Y%m"), rand_suffix())
}

fn rand_suffix() -> u32 {
    use rand::Rng;
    rand::thread_rng().gen_range(100000..999999)
}

// ── Network usage (user) ──────────────────────────────────────────────────────

pub async fn get_project_network_usage(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Path(project_id): Path<String>,
) -> AppResult<impl IntoResponse> {
    super::projects::check_project_access(&state, &auth, &project_id, "VIEWER").await?;

    let rows = sqlx::query!(
        r#"SELECT billing_month, p95_egress_mbps, total_egress_bytes, total_ingress_bytes,
                  mean_req_body_bytes, mean_resp_body_bytes, total_req_count,
                  egress_charge, status, charged_at
           FROM monthly_network_charges
           WHERE project_id = ?
           ORDER BY billing_month DESC
           LIMIT 24"#,
        project_id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|r| {
                serde_json::json!({
                    "billing_month":        r.billing_month,
                    "p95_egress_mbps":      r.p95_egress_mbps,
                    "total_egress_bytes":   r.total_egress_bytes,
                    "total_ingress_bytes":  r.total_ingress_bytes,
                    "mean_req_body_bytes":  r.mean_req_body_bytes,
                    "mean_resp_body_bytes": r.mean_resp_body_bytes,
                    "total_req_count":      r.total_req_count,
                    "egress_charge":        r.egress_charge,
                    "status":               r.status,
                    "charged_at":           r.charged_at,
                })
            })
            .collect::<Vec<_>>(),
    ))
}

pub async fn get_overdue_status(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    let wallet = sqlx::query!(
        r#"SELECT CAST(balance AS DOUBLE) AS balance
           FROM user_wallets WHERE user_id = ?"#,
        auth.user_id
    )
    .fetch_optional(&state.db)
    .await?;

    let balance = wallet.map(|w| w.balance).unwrap_or(0.0_f64);
    let is_overdue = balance < 0.0;

    let charges = if is_overdue {
        sqlx::query!(
            r#"SELECT charge_date, overdue_balance, fee_amount, status
               FROM overdue_charges WHERE user_id = ?
               ORDER BY charge_date DESC LIMIT 30"#,
            auth.user_id
        )
        .fetch_all(&state.db)
        .await?
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "charge_date":     r.charge_date,
                "overdue_balance": r.overdue_balance,
                "fee_amount":      r.fee_amount,
                "status":          r.status,
            })
        })
        .collect::<Vec<_>>()
    } else {
        vec![]
    };

    Ok(Json(serde_json::json!({
        "balance":    balance,
        "is_overdue": is_overdue,
        "charges":    charges,
    })))
}

// ── Admin: monthly network charge management ──────────────────────────────────

#[derive(serde::Deserialize)]
pub struct ApplyChargesRequest {
    pub billing_month: String,
}

pub async fn admin_compute_network_charges(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<ApplyChargesRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("admin only".into()));
    }
    let count = crate::billing::apply_monthly_network_charges(&state, &body.billing_month).await?;
    Ok(Json(
        serde_json::json!({ "computed": count, "billing_month": body.billing_month }),
    ))
}

pub async fn admin_collect_network_charges(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<ApplyChargesRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("admin only".into()));
    }
    let projects =
        crate::billing::collect_monthly_network_charges(&state, &body.billing_month).await?;
    Ok(Json(
        serde_json::json!({ "collected_projects": projects, "count": projects.len() }),
    ))
}

pub async fn admin_list_overdue(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin {
        return Err(AppError::Forbidden("admin only".into()));
    }
    let rows = sqlx::query!(
        r#"SELECT u.username, u.email,
                  CAST(w.balance AS DOUBLE) AS balance,
                  (SELECT charge_date FROM overdue_charges o
                   WHERE o.user_id = w.user_id ORDER BY charge_date ASC LIMIT 1) AS overdue_since
           FROM user_wallets w
           JOIN users u ON u.id = w.user_id
           WHERE w.balance < 0
           ORDER BY w.balance ASC"#
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|r| {
                serde_json::json!({
                    "username":      r.username,
                    "email":         r.email,
                    "balance":       r.balance,
                    "overdue_since": r.overdue_since,
                })
            })
            .collect::<Vec<_>>(),
    ))
}
