//! Usage-based billing: P95 egress, overdue charges, monthly charge application.

use chrono::{NaiveDate, Utc};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    state::AppState,
};

// ── Config helpers ─────────────────────────────────────────────────────────────

async fn cfg(state: &AppState, key: &str, default: f64) -> f64 {
    sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = ?"#,
        key
    )
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse().ok())
    .unwrap_or(default)
}

/// Read the configured billing currency.
///
/// Precedence: `platform_config.billing_currency` → `config.stripe.currency` → "cny".
/// This resolves the dual-config issue: platform_config is authoritative,
/// stripe config is the fallback when platform_config hasn't been set.
pub async fn billing_currency(state: &AppState) -> String {
    sqlx::query_scalar!(r#"SELECT `value` FROM platform_config WHERE `key` = 'billing_currency'"#)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| {
            let sc = &state.config.stripe.currency;
            if sc.is_empty() {
                "cny".to_string()
            } else {
                sc.clone()
            }
        })
}

// ── P95 calculation ────────────────────────────────────────────────────────────

/// Returns the 95th-percentile value from a sorted slice (ascending).
/// Uses nearest-rank method: ceil(0.95 * n).
fn p95(mut samples: Vec<f64>) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((samples.len() as f64 * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(samples.len() - 1);
    samples[idx]
}

// ── Monthly network charge computation ────────────────────────────────────────

pub struct MonthlyNetworkStats {
    pub p95_egress_mbps: f64,
    pub total_egress_bytes: i64,
    pub total_ingress_bytes: i64,
    pub mean_req_body_bytes: i64,
    pub mean_resp_body_bytes: i64,
    pub total_req_count: i64,
    pub egress_charge: f64,
}

/// Compute monthly network stats for a project from lb_bandwidth_samples.
pub async fn compute_monthly_network(
    state: &AppState,
    project_id: &str,
    month_start: NaiveDate, // first day of month
    month_end: NaiveDate,   // first day of NEXT month (exclusive)
) -> AppResult<MonthlyNetworkStats> {
    let rows = sqlx::query!(
        r#"SELECT egress_bytes, ingress_bytes, duration_secs,
                  req_count, req_body_bytes, resp_body_bytes
           FROM lb_bandwidth_samples
           WHERE project_id = ?
             AND sampled_at >= ? AND sampled_at < ?"#,
        project_id,
        month_start.to_string(),
        month_end.to_string(),
    )
    .fetch_all(&state.db)
    .await?;

    if rows.is_empty() {
        return Ok(MonthlyNetworkStats {
            p95_egress_mbps: 0.0,
            total_egress_bytes: 0,
            total_ingress_bytes: 0,
            mean_req_body_bytes: 0,
            mean_resp_body_bytes: 0,
            total_req_count: 0,
            egress_charge: 0.0,
        });
    }

    // P95 of egress bandwidth in Mbps
    let egress_mbps_samples: Vec<f64> = rows
        .iter()
        .map(|r| {
            let dur = r.duration_secs.max(1) as f64;
            r.egress_bytes as f64 / dur * 8.0 / 1_000_000.0
        })
        .collect();
    let p95_egress_mbps = p95(egress_mbps_samples);

    let total_egress_bytes: i64 = rows.iter().map(|r| r.egress_bytes).sum();
    let total_ingress_bytes: i64 = rows.iter().map(|r| r.ingress_bytes).sum();
    let total_req_count: i64 = rows.iter().map(|r| r.req_count as i64).sum();
    let total_req_body: i64 = rows.iter().map(|r| r.req_body_bytes).sum();
    let total_resp_body: i64 = rows.iter().map(|r| r.resp_body_bytes).sum();

    let mean_req_body_bytes = if total_req_count > 0 {
        total_req_body / total_req_count
    } else {
        0
    };
    let mean_resp_body_bytes = if total_req_count > 0 {
        total_resp_body / total_req_count
    } else {
        0
    };

    // Determine charge: P95 model if price_egress_p95_mbps_month > 0, else per-GB
    let price_p95 = cfg(state, "price_egress_p95_mbps_month", 0.0).await;
    let egress_charge = if price_p95 > 0.0 {
        p95_egress_mbps * price_p95
    } else {
        let price_per_gb = cfg(state, "price_egress_gb", 0.08).await;
        total_egress_bytes as f64 / 1_073_741_824.0 * price_per_gb
    };

    Ok(MonthlyNetworkStats {
        p95_egress_mbps,
        total_egress_bytes,
        total_ingress_bytes,
        mean_req_body_bytes,
        mean_resp_body_bytes,
        total_req_count,
        egress_charge,
    })
}

/// Compute and upsert monthly_network_charges for all projects for a given month.
/// Idempotent — safe to re-run.
pub async fn apply_monthly_network_charges(
    state: &AppState,
    billing_month: &str,
) -> AppResult<u32> {
    // Parse month string "YYYY-MM"
    let (year, month): (i32, u32) = {
        let parts: Vec<&str> = billing_month.split('-').collect();
        if parts.len() != 2 {
            return Err(AppError::BadRequest("billing_month must be YYYY-MM".into()));
        }
        (
            parts[0]
                .parse()
                .map_err(|_| AppError::BadRequest("bad year".into()))?,
            parts[1]
                .parse()
                .map_err(|_| AppError::BadRequest("bad month".into()))?,
        )
    };
    let month_start = NaiveDate::from_ymd_opt(year, month, 1)
        .ok_or_else(|| AppError::BadRequest("invalid date".into()))?;
    let month_end = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .ok_or_else(|| AppError::Internal("date overflow".into()))?;

    // All projects that have samples in this month
    let projects = sqlx::query!(
        r#"SELECT DISTINCT s.project_id, p.owner_id
           FROM lb_bandwidth_samples s
           JOIN projects p ON p.id = s.project_id
           WHERE s.sampled_at >= ? AND s.sampled_at < ?"#,
        month_start.to_string(),
        month_end.to_string(),
    )
    .fetch_all(&state.db)
    .await?;

    let mut charged = 0u32;
    for proj in projects {
        let stats =
            compute_monthly_network(state, &proj.project_id, month_start, month_end).await?;
        if stats.egress_charge == 0.0 {
            continue;
        }

        let id = Uuid::new_v4().to_string();
        sqlx::query!(
            r#"INSERT INTO monthly_network_charges
                 (id, project_id, user_id, billing_month,
                  p95_egress_mbps, total_egress_bytes, total_ingress_bytes,
                  mean_req_body_bytes, mean_resp_body_bytes, total_req_count,
                  egress_charge, status)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'PENDING')
               ON DUPLICATE KEY UPDATE
                 p95_egress_mbps      = VALUES(p95_egress_mbps),
                 total_egress_bytes   = VALUES(total_egress_bytes),
                 total_ingress_bytes  = VALUES(total_ingress_bytes),
                 mean_req_body_bytes  = VALUES(mean_req_body_bytes),
                 mean_resp_body_bytes = VALUES(mean_resp_body_bytes),
                 total_req_count      = VALUES(total_req_count),
                 egress_charge        = VALUES(egress_charge)"#,
            id,
            proj.project_id,
            proj.owner_id,
            billing_month,
            stats.p95_egress_mbps,
            stats.total_egress_bytes,
            stats.total_ingress_bytes,
            stats.mean_req_body_bytes,
            stats.mean_resp_body_bytes,
            stats.total_req_count,
            stats.egress_charge,
        )
        .execute(&state.db)
        .await?;

        charged += 1;
    }

    Ok(charged)
}

/// Deduct pending network charges for a billing month from user wallets.
pub async fn collect_monthly_network_charges(
    state: &AppState,
    billing_month: &str,
) -> AppResult<Vec<String>> {
    let charges = sqlx::query!(
        r#"SELECT id, user_id, project_id, egress_charge
           FROM monthly_network_charges
           WHERE billing_month = ? AND status = 'PENDING' AND egress_charge > 0"#,
        billing_month
    )
    .fetch_all(&state.db)
    .await?;

    let currency = billing_currency(state).await;

    let mut collected: Vec<String> = Vec::new();
    for c in charges {
        let amount = {
            use rust_decimal::prelude::ToPrimitive;
            c.egress_charge.to_f64().unwrap_or(0.0)
        };

        // Wrap wallet debit + transaction insert in a SQL transaction
        let mut tx = state.db.begin().await?;
        let tx_id = Uuid::new_v4().to_string();

        // Upsert wallet: deduct or create with negative balance
        sqlx::query!(
            r#"INSERT INTO user_wallets (user_id, balance, currency)
               VALUES (?, -?, ?)
               ON DUPLICATE KEY UPDATE balance = balance - ?"#,
            c.user_id,
            amount,
            currency,
            amount
        )
        .execute(&mut *tx)
        .await?;

        let new_balance: f64 = sqlx::query_scalar!(
            r#"SELECT CAST(balance AS DOUBLE) FROM user_wallets WHERE user_id = ?"#,
            c.user_id
        )
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query!(
            r#"INSERT INTO wallet_transactions
                 (id, user_id, tx_type, amount, balance_after, description, ref_id)
               VALUES (?, ?, 'DEDUCTION', ?, ?, ?, ?)"#,
            tx_id,
            c.user_id,
            -amount,
            new_balance,
            format!("LB network charge {billing_month}"),
            c.id,
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query!(
            r#"UPDATE monthly_network_charges
               SET status = 'CHARGED', charged_at = NOW()
               WHERE id = ?"#,
            c.id,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        collected.push(c.project_id.clone());
        tracing::info!(project_id = %c.project_id, amount, "network charge collected");
    }

    Ok(collected)
}

// ── Overdue charge ─────────────────────────────────────────────────────────────

/// Apply daily overdue fee to users with negative wallet balance.
/// Runs once per day; idempotent for today's date.
pub async fn apply_overdue_charges(state: &AppState) -> AppResult<u32> {
    let enabled: i64 = sqlx::query_scalar!(
        r#"SELECT CAST(`value` AS SIGNED) FROM platform_config WHERE `key` = 'billing_enabled'"#
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or(0);
    if enabled == 0 {
        return Ok(0);
    }

    let fee_pct = cfg(state, "billing_overdue_daily_fee_pct", 0.05).await / 100.0;
    if fee_pct <= 0.0 {
        return Ok(0);
    }

    let today = Utc::now().date_naive();
    let today_str = today.format("%Y-%m-%d").to_string();

    // Users with negative balance that haven't had today's fee yet
    let users = sqlx::query!(
        r#"SELECT w.user_id, CAST(w.balance AS DOUBLE) AS balance
           FROM user_wallets w
           WHERE w.balance < 0
             AND NOT EXISTS (
               SELECT 1 FROM overdue_charges o
               WHERE o.user_id = w.user_id AND o.charge_date = ?
             )"#,
        today_str,
    )
    .fetch_all(&state.db)
    .await?;

    // Fetched once: used to notify each charged user (non-fatal).
    let currency = billing_currency(state).await;
    let platform_name: String = sqlx::query_scalar(
        "SELECT `value` FROM platform_config WHERE `key` = 'platform_display_name'",
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_else(|| "QuickStack".to_string());

    let mut applied = 0u32;
    for u in users {
        let balance = u.balance;
        if balance >= 0.0 {
            continue;
        }

        let fee_amount = (-balance * fee_pct * 10000.0).round() / 10000.0;
        let id = Uuid::new_v4().to_string();
        let tx_id = Uuid::new_v4().to_string();

        // Atomic: overdue record + wallet debit + transaction log
        let mut db_tx = state.db.begin().await?;

        sqlx::query!(
            r#"INSERT IGNORE INTO overdue_charges
                 (id, user_id, charge_date, overdue_balance, fee_pct, fee_amount, status)
               VALUES (?, ?, ?, ?, ?, ?, 'APPLIED')"#,
            id,
            u.user_id,
            today_str,
            balance,
            fee_pct,
            fee_amount,
        )
        .execute(&mut *db_tx)
        .await?;

        sqlx::query!(
            r#"UPDATE user_wallets SET balance = balance - ?, updated_at = NOW()
               WHERE user_id = ?"#,
            fee_amount,
            u.user_id,
        )
        .execute(&mut *db_tx)
        .await?;

        let new_balance: f64 = sqlx::query_scalar!(
            r#"SELECT CAST(balance AS DOUBLE) FROM user_wallets WHERE user_id = ?"#,
            u.user_id
        )
        .fetch_one(&mut *db_tx)
        .await?;

        sqlx::query!(
            r#"INSERT INTO wallet_transactions
                 (id, user_id, tx_type, amount, balance_after, description, ref_id)
               VALUES (?, ?, 'DEDUCTION', ?, ?, 'overdue daily fee', ?)"#,
            tx_id,
            u.user_id,
            -fee_amount,
            new_balance,
            id,
        )
        .execute(&mut *db_tx)
        .await?;

        db_tx.commit().await?;

        applied += 1;
        tracing::warn!(user_id = %u.user_id, balance, fee_amount, "overdue charge applied");

        // Notify the user (non-fatal) so they can top up before apps are suspended.
        let contact: Option<(String, Option<String>)> =
            sqlx::query_as("SELECT email, display_name FROM users WHERE id = ?")
                .bind(&u.user_id)
                .fetch_optional(&state.db)
                .await
                .unwrap_or(None);
        if let Some((email, display_name)) = contact {
            if !email.is_empty() {
                let name = display_name.unwrap_or_else(|| email.clone());
                let _ = state
                    .mailer
                    .send_overdue_notice(
                        &email, &name, fee_amount, new_balance, &currency, &platform_name,
                    )
                    .await;
            }
        }
    }

    Ok(applied)
}

// ── Hourly usage snapshots ─────────────────────────────────────────────────────

/// For every user with running apps or active DBs, write one usage_snapshots row
/// for the current hour (idempotent via UNIQUE KEY uq_usage_user_hour).
/// Computes cost using platform_config pricing keys:
///   price_cpu_mcore_hour  (default 0.0) — cost per mCore per hour
///   price_mem_mb_hour     (default 0.0) — cost per MB RAM per hour
///   price_db_hour         (default 0.0) — cost per active DB instance per hour
pub async fn take_hourly_usage_snapshots(state: &AppState) -> AppResult<u32> {
    // Pricing knobs (all default to 0 so cost stays 0 unless admin configures them)
    let price_cpu = cfg(state, "price_cpu_mcore_hour", 0.0).await;
    let price_mem = cfg(state, "price_mem_mb_hour", 0.0).await;
    let price_db = cfg(state, "price_db_hour", 0.0).await;

    // Aggregate per-user resource usage across running apps
    let app_rows = sqlx::query!(
        r#"SELECT owner_id AS user_id,
                  COUNT(*)                                             AS app_count,
                  COALESCE(SUM(cpu_reservation_mcores * replicas), 0) AS cpu_mcores,
                  COALESCE(SUM(mem_reservation_mb     * replicas), 0) AS mem_mb
           FROM apps
           WHERE status = 'RUNNING'
           GROUP BY owner_id"#
    )
    .fetch_all(&state.db)
    .await?;

    // Aggregate per-user active DB count
    let db_rows = sqlx::query!(
        r#"SELECT created_by AS user_id, COUNT(*) AS db_count
           FROM database_instances
           WHERE status = 'ACTIVE'
           GROUP BY created_by"#
    )
    .fetch_all(&state.db)
    .await?;

    // Merge into a map: user_id → (app_count, cpu, mem, db_count)
    use std::collections::HashMap;
    #[derive(Default)]
    struct UserUsage {
        app_count: u32,
        cpu_mcores: u64,
        mem_mb: u64,
        db_count: u32,
    }
    let mut usage: HashMap<String, UserUsage> = HashMap::new();
    for r in app_rows {
        let e = usage.entry(r.user_id).or_default();
        use rust_decimal::prelude::ToPrimitive;
        e.app_count = r.app_count as u32;
        e.cpu_mcores = r.cpu_mcores.to_u64().unwrap_or(0);
        e.mem_mb = r.mem_mb.to_u64().unwrap_or(0);
    }
    for r in db_rows {
        usage.entry(r.user_id).or_default().db_count = r.db_count as u32;
    }

    if usage.is_empty() {
        return Ok(0);
    }

    let snapshot_time: String =
        sqlx::query_scalar!(r#"SELECT DATE_FORMAT(NOW(), '%Y-%m-%d %H:00:00')"#)
            .fetch_one(&state.db)
            .await?
            .unwrap_or_default();

    let mut written = 0u32;
    for (user_id, u) in &usage {
        let cost = u.cpu_mcores as f64 * price_cpu
            + u.mem_mb as f64 * price_mem
            + u.db_count as f64 * price_db;
        let cost_dec = (cost * 10_000.0).round() / 10_000.0;

        let id = Uuid::new_v4().to_string();
        let rows_affected = sqlx::query!(
            r#"INSERT IGNORE INTO usage_snapshots
                 (id, user_id, snapshot_time,
                  app_count, db_count, cpu_mcores_used, mem_mb_used, storage_gb_used, cost)
               VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?)"#,
            id,
            user_id,
            snapshot_time,
            u.app_count,
            u.db_count,
            u.cpu_mcores,
            u.mem_mb,
            cost_dec,
        )
        .execute(&state.db)
        .await?
        .rows_affected();

        if rows_affected > 0 {
            written += 1;
        }
    }

    if written > 0 {
        tracing::debug!(written, snapshot_time = %snapshot_time, "hourly usage snapshots written");
    }
    Ok(written)
}

// ── Background billing runner ──────────────────────────────────────────────────

/// Runs all periodic billing tasks. Called by main.rs background loop.
pub async fn run_billing_tasks(state: &AppState) {
    // LB stats scrape (every 5 min)
    if let Err(e) = crate::proxy::stats::scrape_and_store(state).await {
        tracing::warn!("lb stats scrape error: {e}");
    }

    // Hourly usage snapshots (idempotent; INSERT IGNORE deduplicates within the hour)
    if let Err(e) = take_hourly_usage_snapshots(state).await {
        tracing::warn!("usage snapshot error: {e}");
    }

    // Overdue charges (once per day, idempotent)
    if let Err(e) = apply_overdue_charges(state).await {
        tracing::warn!("overdue charge error: {e}");
    }

    // Subscription renewals — wallet debit + cycle bump or transition to OVERDUE/EXPIRED.
    // Idempotent: only acts on subs with expires_at <= NOW(), and pushes expires_at on
    // every successful pass so the next call sees an empty due-list.
    if let Err(e) = apply_subscription_renewals(state).await {
        tracing::warn!("subscription renewal error: {e}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// P0.1 + P0.4 — Subscription renewal cron
// ═══════════════════════════════════════════════════════════════════════════════
//
// Runs every 30 minutes from the billing loop. Handles:
//   - ACTIVE subs whose expires_at has passed:
//       * auto_renew=1 + price==0  → push expires_at, status stays ACTIVE
//       * auto_renew=1 + balance OK → debit wallet, push expires_at
//       * auto_renew=1 + balance low → status=OVERDUE, log INSUFFICIENT_FUNDS
//       * auto_renew=0              → status=EXPIRED, reset project quotas
//
// Each attempt is logged to `subscription_renewals` so admins can audit.

/// Apply all due renewals. Idempotent (idempotency comes from comparing
/// expires_at <= NOW() and tightening it on every successful pass).
pub async fn apply_subscription_renewals(state: &AppState) -> AppResult<()> {
    let due: Vec<SubRow> = sqlx::query_as(
        "SELECT s.id, s.user_id, s.plan_id, s.billing_cycle, s.auto_renew, \
                s.expires_at, \
                p.price_monthly, p.price_annually \
         FROM user_subscriptions s \
         JOIN subscription_plans p ON p.id = s.plan_id \
         WHERE s.status = 'ACTIVE' \
           AND s.expires_at IS NOT NULL \
           AND s.expires_at <= NOW()",
    )
    .fetch_all(&state.db)
    .await?;

    if due.is_empty() {
        return Ok(());
    }

    for sub in due {
        if let Err(e) = renew_one(state, &sub).await {
            tracing::warn!(subscription_id = %sub.id, "renewal error: {e}");
            let _ = log_renewal(
                state, &sub, "ERROR",
                rust_decimal::Decimal::ZERO,
                None, None, None,
                Some(&e.to_string()),
            ).await;
        }
    }
    Ok(())
}

async fn renew_one(state: &AppState, sub: &impl SubLike) -> AppResult<()> {
    use rust_decimal::Decimal;
    use rust_decimal::prelude::ToPrimitive;

    let cycle = sub.billing_cycle();
    let now_naive = chrono::Utc::now().naive_utc();
    let base = sub.expires_at().unwrap_or(now_naive);

    // ── auto_renew = false → EXPIRED ─────────────────────────────────────────
    if sub.auto_renew() == 0 {
        return mark_expired(state, sub).await;
    }

    // ── auto_renew = true → renewal price + cycle bump ──────────────────────
    let price = match cycle {
        "ANNUALLY" => sub.price_annually().unwrap_or(sub.price_monthly() * Decimal::from(12)),
        "LIFETIME" => Decimal::ZERO, // lifetime subs shouldn't have expires_at, but guard anyway
        _ => sub.price_monthly(),
    };
    let new_expires = match cycle {
        "ANNUALLY" => base + chrono::Duration::days(365),
        "LIFETIME" => base + chrono::Duration::days(36500),
        _ => base + chrono::Duration::days(30),
    };

    if price.is_zero() {
        sqlx::query("UPDATE user_subscriptions SET expires_at = ? WHERE id = ?")
            .bind(new_expires).bind(sub.id())
            .execute(&state.db).await?;
        let _ = log_renewal(
            state, sub, "RENEWED_FREE", Decimal::ZERO,
            None, None, Some(new_expires), None,
        ).await;
        return Ok(());
    }

    // ── Paid renewal: check wallet balance, then deduct atomically ─────────
    let mut tx = state.db.begin().await?;
    let balance: Option<f64> = sqlx::query_scalar(
        "SELECT CAST(balance AS DOUBLE) FROM user_wallets WHERE user_id = ?",
    )
    .bind(sub.user_id())
    .fetch_optional(&mut *tx)
    .await?;
    let balance = balance.unwrap_or(0.0);
    let price_f = price.to_f64().unwrap_or(0.0);

    if balance < price_f {
        // Insufficient — mark OVERDUE, do not push expires_at
        sqlx::query(
            "UPDATE user_subscriptions SET status = 'OVERDUE' WHERE id = ? AND status = 'ACTIVE'",
        )
        .bind(sub.id())
        .execute(&mut *tx).await?;
        tx.commit().await?;
        let _ = log_renewal(
            state, sub, "INSUFFICIENT_FUNDS", price,
            Some(Decimal::try_from(balance).unwrap_or(Decimal::ZERO)),
            Some(Decimal::try_from(balance).unwrap_or(Decimal::ZERO)),
            None,
            Some(&format!("balance {balance} < price {price_f}")),
        ).await;
        tracing::info!(
            subscription_id = %sub.id(),
            balance, price_f,
            "subscription overdue — insufficient funds"
        );
        return Ok(());
    }

    // Sufficient — debit + bump expires_at + log transaction
    let currency = billing_currency(state).await;
    sqlx::query(
        "INSERT INTO user_wallets (user_id, balance, currency) \
         VALUES (?, -?, ?) \
         ON DUPLICATE KEY UPDATE balance = balance - ?",
    )
    .bind(sub.user_id()).bind(price_f).bind(&currency).bind(price_f)
    .execute(&mut *tx).await?;

    let new_balance: f64 = sqlx::query_scalar(
        "SELECT CAST(balance AS DOUBLE) FROM user_wallets WHERE user_id = ?",
    )
    .bind(sub.user_id())
    .fetch_one(&mut *tx).await?;

    let tx_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO wallet_transactions \
            (id, user_id, tx_type, amount, balance_after, description, ref_id) \
         VALUES (?, ?, 'DEDUCTION', ?, ?, ?, ?)",
    )
    .bind(&tx_id).bind(sub.user_id())
    .bind(-price_f).bind(new_balance)
    .bind(format!("Subscription renewal ({cycle})"))
    .bind(sub.id())
    .execute(&mut *tx).await?;

    sqlx::query(
        "UPDATE user_subscriptions SET expires_at = ?, status = 'ACTIVE' WHERE id = ?",
    )
    .bind(new_expires).bind(sub.id())
    .execute(&mut *tx).await?;
    tx.commit().await?;

    let _ = log_renewal(
        state, sub, "CHARGED", price,
        Some(Decimal::try_from(balance).unwrap_or(Decimal::ZERO)),
        Some(Decimal::try_from(new_balance).unwrap_or(Decimal::ZERO)),
        Some(new_expires),
        None,
    ).await;
    tracing::info!(
        subscription_id = %sub.id(),
        amount = price_f, new_balance,
        "subscription renewed"
    );
    Ok(())
}

async fn mark_expired(state: &AppState, sub: &impl SubLike) -> AppResult<()> {
    sqlx::query(
        "UPDATE user_subscriptions SET status = 'EXPIRED' WHERE id = ? AND status = 'ACTIVE'",
    )
    .bind(sub.id())
    .execute(&state.db).await?;

    // Reset all project quotas owned by this user — quota enforcer will then
    // scale down apps that exceed the zeroed limits on its next pass.
    sqlx::query(
        "UPDATE projects SET \
            quota_cpu_mcores = 0, quota_mem_mb = 0, quota_storage_gb = 0, \
            quota_bandwidth_gb = 0, quota_domain_count = 0, \
            quota_db_instances = 0, quota_apps = 0, quota_request_million = 0, \
            quota_mq_bindings = 0, quota_smtp_bindings = 0, \
            quota_redis_bindings = 0, quota_s3_bindings = 0 \
         WHERE owner_id = ?",
    )
    .bind(sub.user_id())
    .execute(&state.db).await?;

    let _ = log_renewal(state, sub, "EXPIRED", rust_decimal::Decimal::ZERO, None, None, None, None).await;
    tracing::info!(subscription_id = %sub.id(), "subscription expired (auto_renew off)");
    Ok(())
}

async fn log_renewal(
    state: &AppState,
    sub: &impl SubLike,
    status: &str,
    amount: rust_decimal::Decimal,
    balance_before: Option<rust_decimal::Decimal>,
    balance_after: Option<rust_decimal::Decimal>,
    new_expires_at: Option<chrono::NaiveDateTime>,
    error_message: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO subscription_renewals \
            (id, subscription_id, user_id, plan_id, billing_cycle, \
             amount, status, balance_before, balance_after, \
             previous_expires_at, new_expires_at, error_message) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(sub.id())
    .bind(sub.user_id())
    .bind(sub.plan_id())
    .bind(sub.billing_cycle())
    .bind(amount)
    .bind(status)
    .bind(balance_before)
    .bind(balance_after)
    .bind(sub.expires_at())
    .bind(new_expires_at)
    .bind(error_message)
    .execute(&state.db).await?;
    Ok(())
}

/// Tiny trait so renew_one/mark_expired can be called with the local Sub
/// struct without exposing it publicly. Avoids re-deriving FromRow on a
/// public type.
trait SubLike {
    fn id(&self) -> &str;
    fn user_id(&self) -> &str;
    fn plan_id(&self) -> &str;
    fn billing_cycle(&self) -> &str;
    fn auto_renew(&self) -> i8;
    fn expires_at(&self) -> Option<chrono::NaiveDateTime>;
    fn price_monthly(&self) -> rust_decimal::Decimal;
    fn price_annually(&self) -> Option<rust_decimal::Decimal>;
}

// Manual impl for the inline `Sub` struct above (using nested-scope trick via
// a free-standing impl block at module scope).
impl SubLike for SubRow {
    fn id(&self) -> &str { &self.id }
    fn user_id(&self) -> &str { &self.user_id }
    fn plan_id(&self) -> &str { &self.plan_id }
    fn billing_cycle(&self) -> &str { &self.billing_cycle }
    fn auto_renew(&self) -> i8 { self.auto_renew }
    fn expires_at(&self) -> Option<chrono::NaiveDateTime> { self.expires_at }
    fn price_monthly(&self) -> rust_decimal::Decimal { self.price_monthly }
    fn price_annually(&self) -> Option<rust_decimal::Decimal> { self.price_annually }
}

#[derive(sqlx::FromRow)]
struct SubRow {
    id: String,
    user_id: String,
    plan_id: String,
    billing_cycle: String,
    auto_renew: i8,
    expires_at: Option<chrono::NaiveDateTime>,
    price_monthly: rust_decimal::Decimal,
    price_annually: Option<rust_decimal::Decimal>,
}
