//! Step-level progress tracker for node provisioning.
//!
//! Each SSH command is wrapped in a "step" with a human-readable name.
//! Output is captured incrementally and flushed to `node_provision_logs`.
//! The logger checks a cancel flag between steps so admins can abort.

use sqlx::MySqlPool;

use crate::error::{AppError, AppResult};

/// Tracks provisioning progress for a single node attempt.
#[derive(Clone)]
pub struct ProvisionLogger {
    db: MySqlPool,
    pub node_id: String,
    pub attempt: u32,
}

impl ProvisionLogger {
    /// Start a new provision attempt. Increments the attempt counter on the node
    /// and clears the cancel flag.
    pub async fn begin(db: MySqlPool, node_id: &str) -> AppResult<Self> {
        // Increment attempt counter, clear cancel flag, set status
        let attempt: u32 = sqlx::query_scalar::<_, Option<u32>>(
            "SELECT provision_attempt FROM cluster_nodes WHERE id = ?",
        )
        .bind(node_id)
        .fetch_optional(&db)
        .await?
        .flatten()
        .unwrap_or(0)
        + 1;

        sqlx::query(
            "UPDATE cluster_nodes SET provision_attempt = ?, provision_cancel = 0, \
             provision_step = 'starting', node_status = 'PROVISIONING', provision_error = NULL \
             WHERE id = ?",
        )
        .bind(attempt)
        .bind(node_id)
        .execute(&db)
        .await?;

        Ok(Self {
            db,
            node_id: node_id.to_string(),
            attempt,
        })
    }

    /// Check if the admin has requested cancellation.
    pub async fn is_cancelled(&self) -> bool {
        sqlx::query_scalar::<_, i8>(
            "SELECT provision_cancel FROM cluster_nodes WHERE id = ?",
        )
        .bind(&self.node_id)
        .fetch_optional(&self.db)
        .await
        .ok()
        .flatten()
        .unwrap_or(0)
        != 0
    }

    /// Insert a new step row with status RUNNING. Returns the row id.
    pub async fn step_begin(&self, index: u16, name: &str) -> AppResult<i64> {
        // Update current step on the node row (for quick status polls)
        sqlx::query("UPDATE cluster_nodes SET provision_step = ? WHERE id = ?")
            .bind(name)
            .bind(&self.node_id)
            .execute(&self.db)
            .await?;

        let result = sqlx::query(
            "INSERT INTO node_provision_logs (node_id, attempt, step_index, step_name, status) \
             VALUES (?, ?, ?, ?, 'RUNNING')",
        )
        .bind(&self.node_id)
        .bind(self.attempt)
        .bind(index)
        .bind(name)
        .execute(&self.db)
        .await?;

        Ok(result.last_insert_id() as i64)
    }

    /// Append output text to a running step (incremental flush).
    pub async fn step_append_output(&self, log_id: i64, chunk: &str) {
        let _ = sqlx::query(
            "UPDATE node_provision_logs SET output = CONCAT(COALESCE(output, ''), ?) WHERE id = ?",
        )
        .bind(chunk)
        .bind(log_id)
        .execute(&self.db)
        .await;
    }

    /// Mark a step as completed (OK, FAILED, SKIPPED, CANCELLED).
    pub async fn step_finish(&self, log_id: i64, status: &str, final_output: Option<&str>) {
        if let Some(out) = final_output {
            let _ = sqlx::query(
                "UPDATE node_provision_logs \
                 SET status = ?, output = CONCAT(COALESCE(output, ''), ?), finished_at = NOW() \
                 WHERE id = ?",
            )
            .bind(status)
            .bind(out)
            .bind(log_id)
            .execute(&self.db)
            .await;
        } else {
            let _ = sqlx::query(
                "UPDATE node_provision_logs SET status = ?, finished_at = NOW() WHERE id = ?",
            )
            .bind(status)
            .bind(log_id)
            .execute(&self.db)
            .await;
        }
    }

    /// Mark the entire provision as finished (updates node row).
    pub async fn finish_ok(&self) {
        let _ = sqlx::query(
            "UPDATE cluster_nodes SET provision_step = 'done' WHERE id = ?",
        )
        .bind(&self.node_id)
        .execute(&self.db)
        .await;
    }

    /// Return the highest step_index that completed with status OK in the
    /// **previous** attempt. Returns 0 if no previous attempt or no OK steps.
    /// Used by reprovision to skip already-completed steps.
    pub async fn last_ok_step(&self) -> u16 {
        if self.attempt <= 1 {
            return 0;
        }
        sqlx::query_scalar::<_, Option<i16>>(
            "SELECT MAX(step_index) FROM node_provision_logs \
             WHERE node_id = ? AND attempt = ? AND status = 'OK'",
        )
        .bind(&self.node_id)
        .bind(self.attempt - 1)
        .fetch_optional(&self.db)
        .await
        .ok()
        .flatten()
        .flatten()
        .map(|v| v.max(0) as u16)
        .unwrap_or(0)
    }

    /// Mark the provision as failed with an error message.
    pub async fn finish_err(&self, err: &str) {
        let _ = sqlx::query(
            "UPDATE cluster_nodes SET provision_step = 'failed', \
             node_status = 'NOT_READY', provision_error = ? WHERE id = ?",
        )
        .bind(err)
        .bind(&self.node_id)
        .execute(&self.db)
        .await;
    }
}
