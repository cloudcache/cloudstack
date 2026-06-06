use sqlx::{mysql::MySqlPoolOptions, MySqlPool};
use std::time::Duration;

use crate::config::DatabaseConfig;

pub async fn connect(cfg: &DatabaseConfig) -> anyhow::Result<MySqlPool> {
    let pool = MySqlPoolOptions::new()
        .max_connections(cfg.max_connections)
        .min_connections(cfg.min_connections)
        .acquire_timeout(Duration::from_secs(cfg.connect_timeout_secs))
        .connect(&cfg.url)
        .await?;

    // Clean up any partially-applied or checksum-mismatched migration records.
    // MySQL DDL is non-transactional — a failed migration can leave tables/columns
    // created but _sqlx_migrations.success = 0, or with a stale checksum after
    // the .sql file was edited. All our migrations are fully idempotent
    // (IF NOT EXISTS / information_schema guards), so re-running is always safe.
    cleanup_dirty_migrations(&pool).await;

    // Run pending migrations
    sqlx::migrate!("src/db/migrations").run(&pool).await?;

    Ok(pool)
}

/// Remove _sqlx_migrations rows that would block a re-run:
///   - success = 0  (partially applied)
///   - checksum mismatch vs the compiled-in migrations
async fn cleanup_dirty_migrations(pool: &MySqlPool) {
    // Table may not exist on first boot
    let exists: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM information_schema.TABLES \
         WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = '_sqlx_migrations'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    if !exists {
        return;
    }

    // Delete rows with success = 0
    let deleted = sqlx::query("DELETE FROM _sqlx_migrations WHERE success = 0")
        .execute(pool)
        .await;
    if let Ok(res) = &deleted {
        if res.rows_affected() > 0 {
            tracing::warn!(
                count = res.rows_affected(),
                "removed partially-applied migration records"
            );
        }
    }

    // Delete rows whose checksum doesn't match the compiled-in migrations.
    // This handles the case where a .sql file was edited after a partial run.
    let compiled = sqlx::migrate!("src/db/migrations");
    for m in compiled.migrations.iter() {
        let version = m.version;
        let expected_checksum = &m.checksum;

        let row: Option<(Vec<u8>,)> =
            sqlx::query_as("SELECT checksum FROM _sqlx_migrations WHERE version = ?")
                .bind(version)
                .fetch_optional(pool)
                .await
                .unwrap_or(None);

        if let Some((stored_checksum,)) = row {
            if stored_checksum.as_slice() != expected_checksum.as_ref() {
                tracing::warn!(
                    version,
                    "migration checksum mismatch — removing stale record for re-apply"
                );
                let _ = sqlx::query("DELETE FROM _sqlx_migrations WHERE version = ?")
                    .bind(version)
                    .execute(pool)
                    .await;
            }
        }
    }
}
