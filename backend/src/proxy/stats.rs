//! LB bandwidth sample collector — polls Pingora /stats/domains every 5 minutes
//! and stores per-app egress/ingress/request counters in lb_bandwidth_samples.

use chrono::{DateTime, Timelike, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::{error::AppResult, state::AppState};

/// Shape returned by Pingora GET /stats/domains
#[derive(Debug, Deserialize)]
pub struct DomainStat {
    pub domain: String,
    /// Bytes received from clients (inbound to proxy)
    pub ingress_bytes: i64,
    /// Bytes sent to clients (outbound from proxy)
    pub egress_bytes: i64,
    pub req_count: i64,
    pub req_body_bytes: i64,
    pub resp_body_bytes: i64,
}

/// Truncate a timestamp to the nearest 5-minute boundary.
fn floor_5min(ts: DateTime<Utc>) -> DateTime<Utc> {
    let min = ts.minute() - (ts.minute() % 5);
    ts.with_minute(min).unwrap().with_second(0).unwrap().with_nanosecond(0).unwrap()
}

/// Fetch stats from Pingora and write samples to the DB.
/// Skips silently if no Pingora client is configured.
pub async fn scrape_and_store(state: &AppState) -> AppResult<()> {
    let pingora = state.pingora.read().await;
    let Some(client) = pingora.as_ref() else {
        return Ok(());
    };

    let stats: Vec<DomainStat> = match client.get_domain_stats().await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("lb stats scrape failed: {e}");
            return Ok(());
        }
    };

    let sampled_at = floor_5min(Utc::now());
    let sampled_at_str = sampled_at.format("%Y-%m-%d %H:%M:%S").to_string();

    for stat in &stats {
        // Resolve domain → app_id
        let app_id = sqlx::query_scalar!(
            r#"SELECT app_id FROM app_domains WHERE hostname = ? LIMIT 1"#,
            stat.domain
        )
        .fetch_optional(&state.db)
        .await?;

        let Some(app_id) = app_id else { continue };

        let project_id = sqlx::query_scalar!(
            r#"SELECT project_id FROM apps WHERE id = ? LIMIT 1"#,
            app_id
        )
        .fetch_optional(&state.db)
        .await?;

        let Some(project_id) = project_id else { continue };

        let id = Uuid::new_v4().to_string();
        sqlx::query!(
            r#"INSERT INTO lb_bandwidth_samples
                 (id, app_id, project_id, sampled_at, duration_secs,
                  ingress_bytes, egress_bytes, req_count, req_body_bytes, resp_body_bytes)
               VALUES (?, ?, ?, ?, 300, ?, ?, ?, ?, ?)
               ON DUPLICATE KEY UPDATE
                 ingress_bytes   = ingress_bytes   + VALUES(ingress_bytes),
                 egress_bytes    = egress_bytes    + VALUES(egress_bytes),
                 req_count       = req_count       + VALUES(req_count),
                 req_body_bytes  = req_body_bytes  + VALUES(req_body_bytes),
                 resp_body_bytes = resp_body_bytes + VALUES(resp_body_bytes)"#,
            id, app_id, project_id, sampled_at_str,
            stat.ingress_bytes, stat.egress_bytes,
            stat.req_count, stat.req_body_bytes, stat.resp_body_bytes,
        )
        .execute(&state.db)
        .await?;
    }

    Ok(())
}
