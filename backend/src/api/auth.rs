use axum::{extract::State, response::IntoResponse, Extension, Json};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use totp_rs::{Algorithm, Secret, TOTP};
use uuid::Uuid;

use crate::{
    auth::{jwt, middleware::AuthUser},
    crypto::CryptoService,
    error::{AppError, AppResult},
    state::AppState,
};

// ─── Login ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
    pub totp_code: Option<String>,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub refresh_token: String,
    pub user: UserInfo,
}

#[derive(Serialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub email: String,
    pub display_name: Option<String>,
    pub is_global_admin: bool,
}

pub async fn login(
    State(state): State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    headers: axum::http::HeaderMap,
    Json(body): Json<LoginRequest>,
) -> AppResult<impl IntoResponse> {
    // ── 1. Try local password auth first ─────────────────────────────────────
    // Accept username OR email in the login field
    let local_user = sqlx::query!(
        r#"SELECT id, username, email, display_name, is_global_admin, is_active, password_hash
           FROM users WHERE username = ? OR email = ?"#,
        body.username,
        body.username
    )
    .fetch_optional(&state.db)
    .await?;

    let user_id: String;

    if let Some(ref row) = local_user {
        if let Some(ref hash) = row.password_hash {
            // User has a local password — verify with argon2
            if verify_password(&body.password, hash) {
                user_id = row.id.clone();
            } else {
                return Err(AppError::Unauthorized("invalid credentials".into()));
            }
        } else {
            // User exists but has no local password — try LDAP
            user_id = try_ldap_login(&state, &body).await?;
        }
    } else {
        // User not in DB — try LDAP (will create the user row)
        user_id = try_ldap_login(&state, &body).await?;
    }

    // Ensure wallet exists
    sqlx::query!(
        r#"INSERT IGNORE INTO user_wallets (user_id) VALUES (?)"#,
        user_id
    )
    .execute(&state.db)
    .await?;

    // Check account active
    let user = sqlx::query!(
        r#"SELECT id, username, email, display_name, is_global_admin, is_active
           FROM users WHERE id = ?"#,
        user_id
    )
    .fetch_one(&state.db)
    .await?;

    if user.is_active == 0 {
        return Err(AppError::Forbidden("account disabled".into()));
    }

    // TOTP verification (if enabled)
    let totp_row = sqlx::query!(
        r#"SELECT secret, enabled FROM totp_credentials WHERE user_id = ?"#,
        user_id
    )
    .fetch_optional(&state.db)
    .await?;

    if let Some(totp) = totp_row.filter(|t| t.enabled != 0) {
        let code = body
            .totp_code
            .as_deref()
            .ok_or_else(|| AppError::Unauthorized("TOTP_REQUIRED".into()))?;

        let secret_plain = state.crypto.decrypt(&totp.secret)?;
        let totp_instance = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            Secret::Raw(secret_plain.into_bytes()).to_bytes().unwrap(),
            None,
            "".to_string(),
        )
        .map_err(|e| AppError::Crypto(e.to_string()))?;

        if !totp_instance.check_current(code).unwrap_or(false) {
            return Err(AppError::Unauthorized("TOTP_INVALID".into()));
        }
    }

    let tokens = issue_session(&state, &user_id, &addr, &headers).await?;

    Ok(Json(LoginResponse {
        token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        user: UserInfo {
            id: user.id,
            username: user.username,
            email: user.email,
            display_name: user.display_name,
            is_global_admin: user.is_global_admin != 0,
        },
    }))
}

/// Authenticate via LDAP and upsert the user row. Returns the user ID.
async fn try_ldap_login(state: &AppState, body: &LoginRequest) -> AppResult<String> {
    use crate::auth::ldap::LdapService;

    let ldap = LdapService::new(state.config.ldap.clone());
    let ldap_user = ldap.authenticate(&body.username, &body.password).await?;

    // Match by DN/uid/email. A partial or multi-row match is unsafe because it
    // would bind one LDAP identity to the wrong local account.
    let matches = sqlx::query(
        r#"SELECT id, username, email, ldap_dn
           FROM users
           WHERE ldap_dn = ? OR username = ? OR email = ?"#,
    )
    .bind(&ldap_user.dn)
    .bind(&ldap_user.uid)
    .bind(&ldap_user.email)
    .fetch_all(&state.db)
    .await?;

    if matches.len() > 1 {
        return Err(AppError::Conflict(
            "LDAP identity matches multiple local users; run LDAP sync and resolve conflicts".into(),
        ));
    }

    if let Some(row) = matches.first() {
        let id: String = sqlx::Row::get(row, "id");
        let username: String = sqlx::Row::get(row, "username");
        let email: String = sqlx::Row::get(row, "email");
        let ldap_dn: Option<String> = sqlx::Row::get(row, "ldap_dn");

        if ldap_dn.is_none()
            && (username != ldap_user.uid || !email.eq_ignore_ascii_case(&ldap_user.email))
        {
            return Err(AppError::Conflict(
                "LDAP uid/mail must match the same local user before binding".into(),
            ));
        }

        // LDAP can promote to admin but never demote — preserve DB-level grants.
        if ldap_user.is_admin {
            sqlx::query!(
                r#"UPDATE users
                   SET email = ?, display_name = ?, ldap_dn = ?,
                       ldap_uid = ?, ldap_gid = ?, is_global_admin = 1,
                       updated_at = NOW()
                   WHERE id = ?"#,
                ldap_user.email,
                ldap_user.display_name,
                ldap_user.dn,
                ldap_user.posix_uid,
                ldap_user.posix_gid,
                id,
            )
            .execute(&state.db)
            .await?;
        } else {
            sqlx::query!(
                r#"UPDATE users
                   SET email = ?, display_name = ?, ldap_dn = ?,
                       ldap_uid = ?, ldap_gid = ?,
                       updated_at = NOW()
                   WHERE id = ?"#,
                ldap_user.email,
                ldap_user.display_name,
                ldap_user.dn,
                ldap_user.posix_uid,
                ldap_user.posix_gid,
                id,
            )
            .execute(&state.db)
            .await?;
        }
        Ok(id)
    } else {
        let new_id = Uuid::new_v4().to_string();
        sqlx::query!(
            r#"INSERT INTO users
               (id, username, email, display_name, ldap_dn, ldap_uid, ldap_gid, is_global_admin)
               VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
            new_id,
            ldap_user.uid,
            ldap_user.email,
            ldap_user.display_name,
            ldap_user.dn,
            ldap_user.posix_uid,
            ldap_user.posix_gid,
            ldap_user.is_admin as i8,
        )
        .execute(&state.db)
        .await?;
        Ok(new_id)
    }
}

// ─── Register ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    pub display_name: Option<String>,
}

pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> AppResult<impl IntoResponse> {
    // Check platform setting
    let enabled = sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = 'registration_enabled'"#
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_else(|| "1".to_string());

    if enabled != "1" {
        return Err(AppError::Forbidden("self-registration is disabled".into()));
    }

    // Basic validation
    if body.username.len() < 3 || body.username.len() > 64 {
        return Err(AppError::BadRequest(
            "username must be 3–64 characters".into(),
        ));
    }
    if body.password.len() < 8 {
        return Err(AppError::BadRequest(
            "password must be at least 8 characters".into(),
        ));
    }
    if !body.email.contains('@') {
        return Err(AppError::BadRequest("invalid email".into()));
    }

    // Check duplicate username/email in local DB
    let exists = sqlx::query_scalar!(
        r#"SELECT COUNT(*) FROM users WHERE username = ? OR email = ?"#,
        body.username,
        body.email
    )
    .fetch_one(&state.db)
    .await?;

    if exists > 0 {
        return Err(AppError::Conflict(
            "username or email already registered".into(),
        ));
    }

    // Try LLDAP user creation (non-fatal — works without LDAP configured)
    let _ = state
        .lldap
        .create_user(
            &body.username,
            &body.email,
            body.display_name.as_deref(),
            &body.password,
        )
        .await;

    // If a default group is configured, add user to it
    if let Some(group_id) = state.config.ldap.default_user_group_id {
        let _ = state
            .lldap
            .add_user_to_group(&body.username, group_id)
            .await;
    }

    // First registered user becomes admin automatically
    let user_count = sqlx::query_scalar!(r#"SELECT COUNT(*) FROM users"#)
        .fetch_one(&state.db)
        .await?;
    let is_first_user = user_count == 0;

    // Insert local user row
    let id = Uuid::new_v4().to_string();
    let require_approval = if is_first_user {
        // First user is always active, no approval needed
        "0".to_string()
    } else {
        sqlx::query_scalar!(
            r#"SELECT `value` FROM platform_config WHERE `key` = 'registration_require_approval'"#
        )
        .fetch_optional(&state.db)
        .await?
        .unwrap_or_else(|| "0".to_string())
    };

    let is_active: i8 = if require_approval == "1" { 0 } else { 1 };
    let is_admin: i8 = if is_first_user { 1 } else { 0 };

    let password_hash = hash_password(&body.password)?;

    sqlx::query!(
        r#"INSERT INTO users (id, username, email, display_name, is_active, is_global_admin, password_hash)
           VALUES (?, ?, ?, ?, ?, ?, ?)"#,
        id,
        body.username,
        body.email,
        body.display_name,
        is_active,
        is_admin,
        password_hash,
    )
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref de) if de.is_unique_violation() => {
            AppError::Conflict("username or email already registered".into())
        }
        other => AppError::Database(other),
    })?;

    sqlx::query!(
        r#"INSERT IGNORE INTO user_wallets (user_id) VALUES (?)"#,
        id
    )
    .execute(&state.db)
    .await?;

    // Send welcome email (non-fatal)
    let platform_name = sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = 'platform_display_name'"#
    )
    .fetch_optional(&state.db)
    .await?
    .unwrap_or_else(|| "QuickStack".to_string());

    let login_url = format!("{}/login", state.config.server.host);
    let _ = state
        .mailer
        .send_registration_success(&body.email, &body.username, &login_url, &platform_name)
        .await;

    let msg = if require_approval == "1" {
        "注册成功，等待管理员审批后即可登录"
    } else {
        "注册成功"
    };

    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "id": id, "message": msg })),
    ))
}

// ─── Forgot password ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

pub async fn forgot_password(
    State(state): State<AppState>,
    Json(body): Json<ForgotPasswordRequest>,
) -> AppResult<impl IntoResponse> {
    // Always return 200 to avoid user enumeration
    let user = sqlx::query!(
        r#"SELECT id, username, email FROM users WHERE email = ? AND is_active = 1"#,
        body.email
    )
    .fetch_optional(&state.db)
    .await?;

    if let Some(user) = user {
        // Invalidate any existing unused tokens for this user
        sqlx::query!(
            r#"DELETE FROM password_reset_tokens WHERE user_id = ? AND used_at IS NULL"#,
            user.id
        )
        .execute(&state.db)
        .await?;

        // Generate token
        let raw_token = generate_secure_token();
        let token_hash = CryptoService::sha256_hex(&raw_token);
        let expires_at = Utc::now() + Duration::hours(1);

        sqlx::query!(
            r#"INSERT INTO password_reset_tokens (id, user_id, token_hash, expires_at)
               VALUES (?, ?, ?, ?)"#,
            Uuid::new_v4().to_string(),
            user.id,
            token_hash,
            expires_at.naive_utc(),
        )
        .execute(&state.db)
        .await?;

        // Build reset URL (frontend handles the form)
        let frontend_url = sqlx::query_scalar!(
            r#"SELECT `value` FROM platform_config WHERE `key` = 'frontend_url'"#
        )
        .fetch_optional(&state.db)
        .await?
        .unwrap_or_else(|| "http://localhost:3000".to_string());

        let reset_url = format!("{frontend_url}/reset-password?token={raw_token}");

        let platform_name = sqlx::query_scalar!(
            r#"SELECT `value` FROM platform_config WHERE `key` = 'platform_display_name'"#
        )
        .fetch_optional(&state.db)
        .await?
        .unwrap_or_else(|| "QuickStack".to_string());

        let _ = state
            .mailer
            .send_password_reset(&user.email, &user.username, &reset_url, &platform_name)
            .await;
    }

    Ok(Json(serde_json::json!({
        "message": "如果该邮箱存在有效账号，重置链接已发送"
    })))
}

// ─── Reset password ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub new_password: String,
}

pub async fn reset_password(
    State(state): State<AppState>,
    Json(body): Json<ResetPasswordRequest>,
) -> AppResult<impl IntoResponse> {
    if body.new_password.len() < 8 {
        return Err(AppError::BadRequest(
            "password must be at least 8 characters".into(),
        ));
    }

    let token_hash = CryptoService::sha256_hex(&body.token);

    let row = sqlx::query!(
        r#"SELECT t.id, t.user_id, t.expires_at, t.used_at, u.username
           FROM password_reset_tokens t
           JOIN users u ON u.id = t.user_id
           WHERE t.token_hash = ?"#,
        token_hash
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("invalid or expired reset token".into()))?;

    if row.used_at.is_some() {
        return Err(AppError::BadRequest("reset token already used".into()));
    }
    if Utc::now().naive_utc() > row.expires_at {
        return Err(AppError::BadRequest("reset token expired".into()));
    }

    // Update local password hash
    let new_hash = hash_password(&body.new_password)?;
    sqlx::query!(
        r#"UPDATE users SET password_hash = ? WHERE id = ?"#,
        new_hash,
        row.user_id
    )
    .execute(&state.db)
    .await?;

    // Try to change password in LLDAP too (non-fatal — user may be local-only)
    let _ = state
        .lldap
        .change_password(&row.username, &body.new_password)
        .await;

    // Mark token used
    sqlx::query!(
        r#"UPDATE password_reset_tokens SET used_at = NOW() WHERE id = ?"#,
        row.id
    )
    .execute(&state.db)
    .await?;

    // Revoke all existing sessions
    sqlx::query!(
        r#"DELETE FROM user_sessions WHERE user_id = ?"#,
        row.user_id
    )
    .execute(&state.db)
    .await?;

    Ok(Json(
        serde_json::json!({ "message": "密码重置成功，请重新登录" }),
    ))
}

// ─── Me ───────────────────────────────────────────────────────────────────────

pub async fn me(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    let user = sqlx::query!(
        r#"SELECT id, username, email, display_name, is_global_admin, created_at
           FROM users WHERE id = ?"#,
        auth.user_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("user not found".into()))?;

    let subscription = sqlx::query!(
        r#"SELECT s.status, p.name AS plan_name, p.display_name AS plan_display_name,
                  s.expires_at
           FROM user_subscriptions s
           JOIN subscription_plans p ON p.id = s.plan_id
           WHERE s.user_id = ? AND s.status IN ('ACTIVE','OVERDUE','PENDING')
           ORDER BY s.created_at DESC LIMIT 1"#,
        auth.user_id
    )
    .fetch_optional(&state.db)
    .await?;

    Ok(Json(serde_json::json!({
        "id": user.id,
        "username": user.username,
        "email": user.email,
        "display_name": user.display_name,
        "is_global_admin": user.is_global_admin != 0,
        "subscription": subscription.map(|s| serde_json::json!({
            "status": s.status,
            "plan_name": s.plan_name,
            "plan_display_name": s.plan_display_name,
            "expires_at": s.expires_at,
        })),
        "created_at": user.created_at,
    })))
}

// ─── Token refresh ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

/// POST /auth/refresh  (public — no auth middleware, access token may be expired)
///
/// Validates the refresh token against the DB, deletes it (rotation),
/// and issues a brand-new access token + refresh token pair.
pub async fn refresh(
    State(state): State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    headers: axum::http::HeaderMap,
    Json(body): Json<RefreshRequest>,
) -> AppResult<impl IntoResponse> {
    let rt_hash = CryptoService::sha256_hex(&body.refresh_token);

    // Validate refresh token: must exist and not be expired.
    // refresh_tokens.expires_at is TIMESTAMP — decode as DateTime<Utc>, not NaiveDateTime.
    #[derive(sqlx::FromRow)]
    struct RtRow { id: String, user_id: String, expires_at: DateTime<Utc> }

    let row: RtRow = sqlx::query_as(
        "SELECT id, user_id, expires_at FROM refresh_tokens WHERE token_hash = ?",
    )
    .bind(&rt_hash)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Unauthorized("invalid refresh token".into()))?;

    if Utc::now() > row.expires_at {
        let _ = sqlx::query("DELETE FROM refresh_tokens WHERE id = ?")
            .bind(&row.id).execute(&state.db).await;
        return Err(AppError::Unauthorized("refresh token expired".into()));
    }

    // Rotation: delete the used refresh token
    let _ = sqlx::query("DELETE FROM refresh_tokens WHERE id = ?")
        .bind(&row.id).execute(&state.db).await;

    // Check user is still active
    let user_active: i8 = sqlx::query_scalar(
        "SELECT is_active FROM users WHERE id = ?",
    )
    .bind(&row.user_id)
    .fetch_optional(&state.db)
    .await?
    .unwrap_or(0);

    if user_active == 0 {
        return Err(AppError::Forbidden("account disabled".into()));
    }

    // Issue new pair
    let tokens = issue_session(&state, &row.user_id, &addr, &headers).await?;

    Ok(Json(serde_json::json!({
        "token": tokens.access_token,
        "refresh_token": tokens.refresh_token,
    })))
}

// ─── Logout ───────────────────────────────────────────────────────────────────

pub async fn logout(
    State(state): State<AppState>,
    axum_extra::TypedHeader(bearer): axum_extra::TypedHeader<
        axum_extra::headers::Authorization<axum_extra::headers::authorization::Bearer>,
    >,
) -> AppResult<impl IntoResponse> {
    let token_hash = CryptoService::sha256_hex(bearer.token());
    // Delete access session
    sqlx::query!(
        r#"DELETE FROM user_sessions WHERE token_hash = ?"#,
        token_hash
    )
    .execute(&state.db)
    .await?;
    // Also clean up any refresh tokens for this user (full logout)
    let secret = state.jwt_secret.read().await.clone();
    if let Ok(claims) = jwt::verify(bearer.token(), &secret) {
        let _ = sqlx::query("DELETE FROM refresh_tokens WHERE user_id = ?")
            .bind(&claims.sub)
            .execute(&state.db)
            .await;
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── TOTP Setup ───────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct TotpSetupResponse {
    pub secret: String,
    pub qr_url: String,
}

pub async fn totp_setup(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    let secret = totp_rs::Secret::generate_secret();
    let secret_str = secret.to_encoded().to_string();

    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret.to_bytes().unwrap(),
        Some("QuickStack".to_string()),
        auth.username.clone(),
    )
    .map_err(|e| AppError::Crypto(e.to_string()))?;

    let qr_url = totp.get_url();
    let encrypted = state.crypto.encrypt(&secret_str)?;

    sqlx::query!(
        r#"INSERT INTO totp_credentials (user_id, secret, enabled)
           VALUES (?, ?, 0)
           ON DUPLICATE KEY UPDATE secret = ?, enabled = 0"#,
        auth.user_id,
        encrypted,
        encrypted,
    )
    .execute(&state.db)
    .await?;

    Ok(Json(TotpSetupResponse {
        secret: secret_str,
        qr_url,
    }))
}

#[derive(Deserialize)]
pub struct TotpVerifyRequest {
    pub code: String,
}

pub async fn totp_verify(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<TotpVerifyRequest>,
) -> AppResult<impl IntoResponse> {
    let row = sqlx::query!(
        r#"SELECT secret FROM totp_credentials WHERE user_id = ? AND enabled = 0"#,
        auth.user_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::BadRequest("no pending TOTP setup".into()))?;

    let secret_plain = state.crypto.decrypt(&row.secret)?;
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        totp_rs::Secret::Raw(secret_plain.into_bytes())
            .to_bytes()
            .unwrap(),
        None,
        "".to_string(),
    )
    .map_err(|e| AppError::Crypto(e.to_string()))?;

    if !totp.check_current(&body.code).unwrap_or(false) {
        return Err(AppError::BadRequest("invalid TOTP code".into()));
    }

    sqlx::query!(
        r#"UPDATE totp_credentials SET enabled = 1 WHERE user_id = ?"#,
        auth.user_id
    )
    .execute(&state.db)
    .await?;

    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn totp_disable(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    sqlx::query!(
        r#"DELETE FROM totp_credentials WHERE user_id = ?"#,
        auth.user_id
    )
    .execute(&state.db)
    .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

pub(crate) struct IssuedTokens {
    pub access_token: String,
    pub refresh_token: String,
}

/// Issue an access JWT + refresh token, persist both. Returns the raw tokens.
pub(crate) async fn issue_session(
    state: &AppState,
    user_id: &str,
    addr: &std::net::SocketAddr,
    headers: &axum::http::HeaderMap,
) -> AppResult<IssuedTokens> {
    let secret = state.jwt_secret.read().await.clone();

    // ── Access token (short-lived JWT) ──────────────────────────────────────
    let access_token = jwt::issue(user_id, &secret, state.config.jwt.expiry_hours)?;
    let access_hash = CryptoService::sha256_hex(&access_token);
    let access_expires = Utc::now() + Duration::hours(state.config.jwt.expiry_hours);

    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    sqlx::query!(
        r#"INSERT INTO user_sessions (id, user_id, token_hash, ip_addr, user_agent, expires_at)
           VALUES (?, ?, ?, ?, ?, ?)"#,
        Uuid::new_v4().to_string(),
        user_id,
        access_hash,
        addr.ip().to_string(),
        user_agent,
        access_expires.naive_utc(),
    )
    .execute(&state.db)
    .await?;

    // ── Refresh token (long-lived opaque token) ─────────────────────────────
    let refresh_raw = format!("qs_rt_{}", Uuid::new_v4().as_simple());
    let refresh_hash = CryptoService::sha256_hex(&refresh_raw);
    let refresh_expires = Utc::now() + Duration::hours(state.config.jwt.refresh_expiry_hours);

    sqlx::query(
        "INSERT INTO refresh_tokens (id, user_id, token_hash, expires_at) VALUES (?, ?, ?, ?)",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(user_id)
    .bind(&refresh_hash)
    // refresh_tokens.expires_at is TIMESTAMP — bind DateTime<Utc>, not NaiveDateTime
    .bind(refresh_expires)
    .execute(&state.db)
    .await?;

    Ok(IssuedTokens {
        access_token,
        refresh_token: refresh_raw,
    })
}

// ─── Registration status (public) ────────────────────────────────────────────

/// GET /auth/registration-status — returns whether self-registration is enabled.
/// Registration is open when:
///   1. No users exist yet (first-user bootstrap), OR
///   2. platform_config `registration_enabled` is explicitly "1" / "true"
/// Default (key absent, users > 0): registration closed.
pub async fn registration_status(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    let user_count = sqlx::query_scalar!(r#"SELECT COUNT(*) FROM users"#)
        .fetch_one(&state.db)
        .await?;

    let first_boot = user_count == 0;

    // No users yet — registration must be open for first admin
    if first_boot {
        return Ok(Json(
            serde_json::json!({ "enabled": true, "first_boot": true }),
        ));
    }

    // After first user, respect the platform_config setting (default: closed)
    let val = sqlx::query_scalar!(
        r#"SELECT `value` FROM platform_config WHERE `key` = 'registration_enabled'"#
    )
    .fetch_optional(&state.db)
    .await?;

    let enabled = val.as_deref() == Some("1") || val.as_deref() == Some("true");
    Ok(Json(
        serde_json::json!({ "enabled": enabled, "first_boot": false }),
    ))
}

fn generate_secure_token() -> String {
    use rand::Rng;
    let bytes: Vec<u8> = rand::thread_rng()
        .sample_iter(rand::distributions::Standard)
        .take(32)
        .collect();
    hex::encode(bytes)
}

/// Hash a password with argon2id (recommended default).
pub(crate) fn hash_password(password: &str) -> AppResult<String> {
    use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
    let salt = SaltString::generate(&mut rand::thread_rng());
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| AppError::Crypto(format!("argon2 hash: {e}")))?;
    Ok(hash.to_string())
}

/// Verify a password against an argon2 PHC string.
fn verify_password(password: &str, hash: &str) -> bool {
    use argon2::{password_hash::PasswordHash, Argon2, PasswordVerifier};
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}
