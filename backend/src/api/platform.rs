use axum::{extract::State, response::IntoResponse, Extension, Json};
use serde::Deserialize;

use crate::{auth::middleware::AuthUser, error::{AppError, AppResult}, state::AppState};

pub async fn list_config(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    let rows = sqlx::query!(r#"SELECT `key`, `value`, description FROM platform_config ORDER BY `key`"#)
        .fetch_all(&state.db)
        .await?;

    // Mask sensitive keys
    let sensitive = ["ldap_bind_password", "ssh_private_key", "jwt_secret"];
    let result: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            let value = if sensitive.contains(&r.key.as_str()) {
                "***".to_string()
            } else {
                r.value
            };
            serde_json::json!({ "key": r.key, "value": value, "description": r.description })
        })
        .collect();

    Ok(Json(result))
}

#[derive(Deserialize)]
pub struct SetConfigRequest {
    pub key: String,
    pub value: String,
}

pub async fn set_config(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<SetConfigRequest>,
) -> AppResult<impl IntoResponse> {
    if !auth.is_global_admin { return Err(AppError::Forbidden("admin only".into())); }

    // Encrypt sensitive fields before storage
    let sensitive = ["ldap_bind_password", "ssh_private_key", "jwt_secret"];
    let stored_value = if sensitive.contains(&body.key.as_str()) {
        state.crypto.encrypt(&body.value)?
    } else {
        body.value.clone()
    };

    sqlx::query!(
        r#"INSERT INTO platform_config (`key`, `value`)
           VALUES (?, ?)
           ON DUPLICATE KEY UPDATE `value` = ?"#,
        body.key, stored_value, stored_value,
    )
    .execute(&state.db)
    .await?;

    // Hot-reload JWT secret if changed
    if body.key == "jwt_secret" {
        *state.jwt_secret.write().await = body.value.clone();
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}
