/// LLDAP HTTP admin client.
///
/// LLDAP exposes a GraphQL API at `{http_url}/api/graphql` and a JWT login
/// endpoint at `{http_url}/auth/simple/login`.  This client handles token
/// caching and exposes the admin operations QuickStack needs.
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::{AppError, AppResult};

// ─── Client ───────────────────────────────────────────────────────────────────

pub struct LldapClient {
    http_url: String,
    admin_username: String,
    admin_password: String,
    /// Cached (token, expiry). Refreshed automatically when within 5 min of expiry.
    token: Arc<RwLock<Option<(String, DateTime<Utc>)>>>,
    client: Client,
}

impl LldapClient {
    pub fn new(http_url: &str, admin_username: &str, admin_password: &str) -> Self {
        Self {
            http_url: http_url.trim_end_matches('/').to_string(),
            admin_username: admin_username.to_string(),
            admin_password: admin_password.to_string(),
            token: Arc::new(RwLock::new(None)),
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("reqwest client"),
        }
    }

    async fn token(&self) -> AppResult<String> {
        {
            let guard = self.token.read().await;
            if let Some((tok, exp)) = guard.as_ref() {
                if Utc::now() < *exp - Duration::minutes(5) {
                    return Ok(tok.clone());
                }
            }
        }

        // Login to obtain a fresh JWT
        #[derive(Deserialize)]
        struct LoginResp {
            token: String,
        }

        let resp = self
            .client
            .post(format!("{}/auth/simple/login", self.http_url))
            .json(&serde_json::json!({
                "username": self.admin_username,
                "password": self.admin_password,
            }))
            .send()
            .await
            .map_err(|e| AppError::Proxy(format!("lldap login: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            return Err(AppError::Proxy(format!("lldap login failed: HTTP {status}")));
        }

        let body: LoginResp = resp
            .json()
            .await
            .map_err(|e| AppError::Proxy(format!("lldap login parse: {e}")))?;

        let expiry = Utc::now() + Duration::hours(23);
        *self.token.write().await = Some((body.token.clone(), expiry));
        Ok(body.token)
    }

    async fn graphql<T: for<'de> Deserialize<'de>>(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> AppResult<T> {
        #[derive(Deserialize)]
        struct GqlResponse<D> {
            data: Option<D>,
            errors: Option<Vec<GqlError>>,
        }
        #[derive(Deserialize)]
        struct GqlError {
            message: String,
        }

        let token = self.token().await?;

        let resp = self
            .client
            .post(format!("{}/api/graphql", self.http_url))
            .bearer_auth(&token)
            .json(&serde_json::json!({ "query": query, "variables": variables }))
            .send()
            .await
            .map_err(|e| AppError::Proxy(format!("lldap graphql: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            return Err(AppError::Proxy(format!("lldap graphql HTTP {status}")));
        }

        let gql: GqlResponse<T> = resp
            .json()
            .await
            .map_err(|e| AppError::Proxy(format!("lldap graphql parse: {e}")))?;

        if let Some(errs) = gql.errors {
            if !errs.is_empty() {
                return Err(AppError::Proxy(format!(
                    "lldap: {}",
                    errs.into_iter().map(|e| e.message).collect::<Vec<_>>().join("; ")
                )));
            }
        }

        gql.data.ok_or_else(|| AppError::Proxy("lldap: empty data".into()))
    }
}

// ─── Admin operations ─────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateUserInput<'a> {
    id: &'a str,
    email: &'a str,
    display_name: Option<&'a str>,
}

impl LldapClient {
    /// Create a new LLDAP user and set their initial password.
    pub async fn create_user(
        &self,
        username: &str,
        email: &str,
        display_name: Option<&str>,
        password: &str,
    ) -> AppResult<()> {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "createUser")]
            create_user: serde_json::Value,
        }

        self.graphql::<Resp>(
            r#"mutation CreateUser($user: CreateUserInput!) {
                 createUser(user: $user) { id }
               }"#,
            serde_json::json!({ "user": {
                "id": username,
                "email": email,
                "displayName": display_name.unwrap_or(username),
            }}),
        )
        .await?;

        // Set password immediately after creation
        self.change_password(username, password).await?;
        Ok(())
    }

    /// Change (or reset) a user's password.
    pub async fn change_password(&self, username: &str, new_password: &str) -> AppResult<()> {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "changeUserPassword")]
            _inner: serde_json::Value,
        }

        self.graphql::<Resp>(
            r#"mutation ChangePassword($userId: String!, $password: String!) {
                 changeUserPassword(userId: $userId, password: $password) { ok }
               }"#,
            serde_json::json!({
                "userId": username,
                "password": new_password,
            }),
        )
        .await?;
        Ok(())
    }

    /// Delete a user from LLDAP.
    pub async fn delete_user(&self, username: &str) -> AppResult<()> {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "deleteUser")]
            _inner: serde_json::Value,
        }

        self.graphql::<Resp>(
            r#"mutation DeleteUser($userId: String!) {
                 deleteUser(userId: $userId) { ok }
               }"#,
            serde_json::json!({ "userId": username }),
        )
        .await?;
        Ok(())
    }

    /// Add a user to an LLDAP group.
    pub async fn add_user_to_group(&self, username: &str, group_id: i64) -> AppResult<()> {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "addUserToGroup")]
            _inner: serde_json::Value,
        }

        self.graphql::<Resp>(
            r#"mutation AddToGroup($userId: String!, $groupId: Int!) {
                 addUserToGroup(userId: $userId, groupId: $groupId) { ok }
               }"#,
            serde_json::json!({ "userId": username, "groupId": group_id }),
        )
        .await?;
        Ok(())
    }

    /// Update a user's display_name and/or email in LLDAP.
    pub async fn update_user(
        &self,
        username: &str,
        display_name: Option<&str>,
        email: Option<&str>,
    ) -> AppResult<()> {
        #[derive(Deserialize)]
        struct Resp {
            #[serde(rename = "updateUser")]
            _inner: serde_json::Value,
        }

        let mut user = serde_json::json!({ "id": username });
        if let Some(dn) = display_name {
            user["displayName"] = serde_json::json!(dn);
        }
        if let Some(em) = email {
            user["email"] = serde_json::json!(em);
        }

        self.graphql::<Resp>(
            r#"mutation UpdateUser($user: UpdateUserInput!) {
                 updateUser(user: $user) { ok }
               }"#,
            serde_json::json!({ "user": user }),
        )
        .await?;
        Ok(())
    }

    /// Simple connectivity check — returns true if admin login succeeds.
    pub async fn health_check(&self) -> bool {
        // Invalidate cached token first so we actually test the connection
        *self.token.write().await = None;
        self.token().await.is_ok()
    }
}
