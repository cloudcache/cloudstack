use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::{AppError, AppResult};

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct CreateHostRequest {
    pub domain: String,
    pub scheme: String,
    pub upstream_ip: String,
    pub upstream_port: u16,
    pub ssl_force: bool,
}

#[derive(Debug, Serialize)]
struct PingoraCreateHostBody {
    domain: String,
    scheme: String,
    upstream: String,
    force_ssl: bool,
}

#[derive(Debug, Serialize)]
struct LoginBody {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct LoginResponse {
    token: String,
}

#[derive(Debug, Serialize)]
struct LocationBody {
    path: String,
    #[serde(rename = "pathRewrite")]
    path_rewrite: String,
    #[serde(rename = "targetUrl")]
    target_url: String,
}

#[derive(Debug, Serialize)]
struct CertRequest {
    domain: String,
}

// ─── Client ───────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct PingoraClient {
    http: HttpClient,
    api_base: String,
    username: String,
    password: String,
    token: Arc<RwLock<String>>,
    token_expiry: Arc<RwLock<Instant>>,
}

impl PingoraClient {
    pub fn new(api_base: &str, username: &str, password: &str) -> Self {
        Self {
            http: HttpClient::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .expect("reqwest client"),
            api_base: api_base.trim_end_matches('/').to_string(),
            username: username.to_string(),
            password: password.to_string(),
            token: Arc::new(RwLock::new(String::new())),
            token_expiry: Arc::new(RwLock::new(Instant::now())),
        }
    }

    async fn refresh_token(&self) -> AppResult<()> {
        let resp = self
            .http
            .post(format!("{}/login", self.api_base))
            .json(&LoginBody {
                username: self.username.clone(),
                password: self.password.clone(),
            })
            .send()
            .await
            .map_err(|e| AppError::Proxy(format!("pingora login: {e}")))?;

        if !resp.status().is_success() {
            return Err(AppError::Proxy(format!(
                "pingora login failed: {}",
                resp.status()
            )));
        }

        let body: LoginResponse = resp
            .json()
            .await
            .map_err(|e| AppError::Proxy(format!("pingora login decode: {e}")))?;

        *self.token.write().await = body.token;
        // pingora JWT is typically valid for 24h; refresh 1h before expiry
        *self.token_expiry.write().await = Instant::now() + Duration::from_secs(23 * 3600);

        Ok(())
    }

    async fn bearer(&self) -> AppResult<String> {
        if Instant::now() >= *self.token_expiry.read().await {
            self.refresh_token().await?;
        }
        Ok(self.token.read().await.clone())
    }

    // ─── Proxy host management ────────────────────────────────────────────

    pub async fn create_host(&self, req: &CreateHostRequest) -> AppResult<()> {
        let token = self.bearer().await?;
        let upstream = format!("{}:{}", req.upstream_ip, req.upstream_port);

        let resp = self
            .http
            .post(format!("{}/hosts", self.api_base))
            .bearer_auth(&token)
            .json(&PingoraCreateHostBody {
                domain: req.domain.clone(),
                scheme: req.scheme.clone(),
                upstream,
                force_ssl: req.ssl_force,
            })
            .send()
            .await
            .map_err(|e| AppError::Proxy(format!("create_host: {e}")))?;

        if !resp.status().is_success() {
            return Err(AppError::Proxy(format!(
                "pingora create_host {} failed: {}",
                req.domain,
                resp.status()
            )));
        }
        Ok(())
    }

    pub async fn delete_host(&self, domain: &str) -> AppResult<()> {
        let token = self.bearer().await?;
        let resp = self
            .http
            .delete(format!("{}/hosts/{domain}", self.api_base))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| AppError::Proxy(format!("delete_host: {e}")))?;

        if !resp.status().is_success() && resp.status() != reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::Proxy(format!(
                "pingora delete_host {domain} failed: {}",
                resp.status()
            )));
        }
        Ok(())
    }

    // ─── Maintenance (app pause) ──────────────────────────────────────────

    /// Redirect all traffic to /_qs/maintenance (503 page on QuickStack itself).
    pub async fn set_maintenance_location(
        &self,
        domain: &str,
    ) -> AppResult<()> {
        let token = self.bearer().await?;

        // QuickStack backend address — derived from master node; hardcoded well-known
        // path since pingora rewrites to it and the backend serves the 503 page.
        let qs_backend = format!("http://localhost:3000");

        let resp = self
            .http
            .post(format!("{}/hosts/{domain}/locations", self.api_base))
            .bearer_auth(&token)
            .json(&LocationBody {
                path: "/".to_string(),
                path_rewrite: "/_qs/maintenance".to_string(),
                target_url: qs_backend,
            })
            .send()
            .await
            .map_err(|e| AppError::Proxy(format!("set_maintenance_location: {e}")))?;

        if !resp.status().is_success() {
            return Err(AppError::Proxy(format!(
                "pingora set_maintenance_location {domain} failed: {}",
                resp.status()
            )));
        }
        Ok(())
    }

    /// Remove maintenance override, restoring normal traffic.
    pub async fn remove_maintenance_location(&self, domain: &str) -> AppResult<()> {
        let token = self.bearer().await?;
        let resp = self
            .http
            .delete(format!("{}/hosts/{domain}/locations", self.api_base))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| AppError::Proxy(format!("remove_maintenance_location: {e}")))?;

        if !resp.status().is_success() && resp.status() != reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::Proxy(format!(
                "pingora remove_maintenance_location {domain} failed: {}",
                resp.status()
            )));
        }
        Ok(())
    }

    // ─── SSL certificates ─────────────────────────────────────────────────

    pub async fn request_cert(&self, domain: &str) -> AppResult<()> {
        let token = self.bearer().await?;
        let resp = self
            .http
            .post(format!("{}/certs", self.api_base))
            .bearer_auth(&token)
            .json(&CertRequest {
                domain: domain.to_string(),
            })
            .send()
            .await
            .map_err(|e| AppError::Proxy(format!("request_cert: {e}")))?;

        if !resp.status().is_success() {
            return Err(AppError::Proxy(format!(
                "pingora request_cert {domain} failed: {}",
                resp.status()
            )));
        }
        Ok(())
    }

    pub async fn list_certs(&self) -> AppResult<serde_json::Value> {
        let token = self.bearer().await?;
        let resp = self
            .http
            .get(format!("{}/certs", self.api_base))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| AppError::Proxy(format!("list_certs: {e}")))?;

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| AppError::Proxy(format!("list_certs decode: {e}")))?;

        Ok(json)
    }

    // ─── Health check ─────────────────────────────────────────────────────

    pub async fn health_check(&self) -> bool {
        self.http
            .get(format!("{}/health", self.api_base))
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Register the platform management domain → QuickStack backend.
    pub async fn init_platform_route(
        &self,
        platform_domain: &str,
        master_ip: &str,
        qs_port: u16,
    ) -> AppResult<()> {
        self.create_host(&CreateHostRequest {
            domain: platform_domain.to_string(),
            scheme: "https".to_string(),
            upstream_ip: master_ip.to_string(),
            upstream_port: qs_port,
            ssl_force: true,
        })
        .await?;

        self.request_cert(platform_domain).await?;
        Ok(())
    }

    /// Fetch per-domain bandwidth counters from Pingora.
    /// Pingora resets counters each time they are read (delta since last poll).
    pub async fn get_domain_stats(&self) -> AppResult<Vec<super::stats::DomainStat>> {
        let token = self.bearer().await?;
        let url = format!("{}/stats/domains", self.api_base);
        let resp = self.http
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("pingora stats: {e}")))?;
        if !resp.status().is_success() {
            return Err(AppError::Internal(format!("pingora stats {}", resp.status())));
        }
        resp.json::<Vec<super::stats::DomainStat>>()
            .await
            .map_err(|e| AppError::Internal(format!("pingora stats parse: {e}")))
    }
}
