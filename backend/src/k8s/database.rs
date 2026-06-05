use k8s_openapi::api::core::v1::Secret;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
use kube::api::{Api, DeleteParams, PostParams};
use std::collections::BTreeMap;

use crate::{error::{AppError, AppResult}, state::AppState};

/// Validates a MySQL identifier (database name or username).
/// Only allows alphanumeric, underscore, and hyphen — rejects backticks, quotes,
/// semicolons, and anything else that could break DDL statement boundaries.
fn validate_mysql_identifier(name: &str, label: &str) -> AppResult<()> {
    if name.is_empty() || name.len() > 64 {
        return Err(AppError::BadRequest(format!("{label} must be 1-64 characters")));
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(AppError::BadRequest(format!(
            "{label} contains invalid characters (only alphanumeric, underscore, hyphen allowed)"
        )));
    }
    Ok(())
}

/// Validates a MySQL password — rejects single quotes and backslashes
/// that could break the IDENTIFIED BY clause.
fn validate_mysql_password(pass: &str) -> AppResult<()> {
    if pass.contains('\'') || pass.contains('\\') {
        return Err(AppError::BadRequest("password contains disallowed characters".into()));
    }
    Ok(())
}

pub async fn provision_database(
    cluster_type: &str,
    host: &str,
    port: u16,
    admin_user: &str,
    admin_pass: &str,
    db_name: &str,
    db_user: &str,
    db_password: &str,
) -> AppResult<()> {
    match cluster_type {
        "MYSQL_GALERA" => provision_mysql(host, port, admin_user, admin_pass, db_name, db_user, db_password).await,
        "POSTGRESQL" => Err(AppError::BadRequest(
            "PostgreSQL provisioning is not yet supported. Only MYSQL_GALERA clusters can be used.".into(),
        )),
        other => Err(AppError::BadRequest(format!("unknown cluster type: {other}"))),
    }
}

async fn provision_mysql(
    host: &str,
    port: u16,
    admin_user: &str,
    admin_pass: &str,
    db_name: &str,
    db_user: &str,
    db_password: &str,
) -> AppResult<()> {
    // Validate all identifiers to prevent SQL injection in DDL statements
    validate_mysql_identifier(db_name, "database name")?;
    validate_mysql_identifier(db_user, "database user")?;
    validate_mysql_password(db_password)?;

    let url = format!("mysql://{}:{}@{}:{}/", admin_user, admin_pass, host, port);
    let pool = sqlx::MySqlPool::connect(&url)
        .await
        .map_err(|e| AppError::Internal(format!("connect to mysql: {e}")))?;

    sqlx::query(&format!("CREATE DATABASE IF NOT EXISTS `{db_name}` CHARACTER SET utf8mb4"))
        .execute(&pool)
        .await
        .map_err(|e| AppError::Internal(format!("create db: {e}")))?;

    sqlx::query(&format!(
        "CREATE USER IF NOT EXISTS '{db_user}'@'%' IDENTIFIED BY '{db_password}'"
    ))
    .execute(&pool)
    .await
    .map_err(|e| AppError::Internal(format!("create user: {e}")))?;

    sqlx::query(&format!(
        "GRANT ALL PRIVILEGES ON `{db_name}`.* TO '{db_user}'@'%'"
    ))
    .execute(&pool)
    .await
    .map_err(|e| AppError::Internal(format!("grant: {e}")))?;

    sqlx::query("FLUSH PRIVILEGES")
        .execute(&pool)
        .await
        .map_err(|e| AppError::Internal(format!("flush: {e}")))?;

    pool.close().await;
    Ok(())
}

pub async fn drop_database(
    cluster_type: &str,
    host: &str,
    port: u16,
    admin_user: &str,
    admin_pass: &str,
    db_name: &str,
    db_user: &str,
) -> AppResult<()> {
    match cluster_type {
        "MYSQL_GALERA" => drop_mysql(host, port, admin_user, admin_pass, db_name, db_user).await,
        other => Err(AppError::Internal(format!(
            "drop_database not implemented for cluster type: {other}"
        ))),
    }
}

async fn drop_mysql(
    host: &str,
    port: u16,
    admin_user: &str,
    admin_pass: &str,
    db_name: &str,
    db_user: &str,
) -> AppResult<()> {
    validate_mysql_identifier(db_name, "database name")?;
    validate_mysql_identifier(db_user, "database user")?;

    let url = format!("mysql://{}:{}@{}:{}/", admin_user, admin_pass, host, port);
    let pool = sqlx::MySqlPool::connect(&url)
        .await
        .map_err(|e| AppError::Internal(format!("connect to mysql: {e}")))?;

    sqlx::query(&format!("DROP DATABASE IF EXISTS `{db_name}`"))
        .execute(&pool)
        .await
        .map_err(|e| AppError::Internal(format!("drop db: {e}")))?;

    sqlx::query(&format!("DROP USER IF EXISTS '{db_user}'@'%'"))
        .execute(&pool)
        .await
        .map_err(|e| AppError::Internal(format!("drop user: {e}")))?;

    pool.close().await;
    Ok(())
}

pub async fn create_db_secret(
    state: &AppState,
    cluster_id: &str,
    namespace: &str,
    secret_name: &str,
    host: &str,
    port: u16,
    db_name: &str,
    db_user: &str,
    db_password: &str,
    cluster_type: &str,
) -> AppResult<()> {
    let client = super::client_for_cluster(state, cluster_id).await?;
    let secret_api: Api<Secret> = Api::namespaced(client, namespace);

    let scheme = if cluster_type.contains("POSTGRESQL") { "postgresql" } else { "mysql" };
    let db_url = format!("{scheme}://{db_user}:{db_password}@{host}:{port}/{db_name}");

    let mut string_data = BTreeMap::new();
    string_data.insert("DB_HOST".to_string(), host.to_string());
    string_data.insert("DB_PORT".to_string(), port.to_string());
    string_data.insert("DB_NAME".to_string(), db_name.to_string());
    string_data.insert("DB_USER".to_string(), db_user.to_string());
    string_data.insert("DB_PASS".to_string(), db_password.to_string());
    string_data.insert("DB_URL".to_string(), db_url);

    let secret = Secret {
        metadata: ObjectMeta {
            name: Some(secret_name.to_string()),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        string_data: Some(string_data),
        ..Default::default()
    };

    secret_api
        .create(&PostParams::default(), &secret)
        .await
        .map_err(|e| AppError::Kubernetes(e))?;

    Ok(())
}

pub async fn delete_db_secret(state: &AppState, cluster_id: &str, namespace: &str, secret_name: &str) -> AppResult<()> {
    let client = super::client_for_cluster(state, cluster_id).await?;
    let secret_api: Api<Secret> = Api::namespaced(client, namespace);
    let _ = secret_api.delete(secret_name, &DeleteParams::default()).await;
    Ok(())
}
