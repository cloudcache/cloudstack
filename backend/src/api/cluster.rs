use axum::{extract::State, response::IntoResponse, Extension, Json};
use serde::Deserialize;

use crate::{auth::middleware::AuthUser, error::{AppError, AppResult}, state::AppState};

/// GET /admin/cluster/storage
pub async fn get_storage(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let storage_root = sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = 'storage_root'"#
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_else(|| "/storage".to_string());

    Ok(Json(serde_json::json!({ "storage_root": storage_root })))
}

#[derive(Deserialize)]
pub struct UpdateStorageRequest {
    pub storage_root: String,
}

/// PUT /admin/cluster/storage
pub async fn update_storage(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<UpdateStorageRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    if body.storage_root.is_empty() || !body.storage_root.starts_with('/') {
        return Err(AppError::BadRequest("storage_root must be an absolute path".into()));
    }

    sqlx::query!(
        r#"INSERT INTO platform_config (`key`, `value`) VALUES ('storage_root', ?)
           ON DUPLICATE KEY UPDATE `value` = VALUES(`value`)"#,
        body.storage_root
    )
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}
