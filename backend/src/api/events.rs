//! Lightweight helper: write rows to `deployment_events`.
//! Fire-and-forget — callers ignore errors so a DB hiccup never blocks the main flow.

use uuid::Uuid;
use crate::state::AppState;

/// Log a deployment lifecycle event.
/// `event_type`: DEPLOY | SCALE | PAUSE | RESUME | CONFIG_CHANGE | ROLLBACK
/// `status`: PENDING | RUNNING | SUCCEEDED | FAILED
pub async fn record(
    state: &AppState,
    app_id: &str,
    event_type: &str,
    status: &str,
    triggered_by: &str,
    message: Option<&str>,
) {
    let id = Uuid::new_v4().to_string();
    let _ = sqlx::query!(
        r#"INSERT INTO deployment_events (id, app_id, event_type, status, triggered_by, message)
           VALUES (?, ?, ?, ?, ?, ?)"#,
        id, app_id, event_type, status, triggered_by, message,
    )
    .execute(&state.db)
    .await;
}
