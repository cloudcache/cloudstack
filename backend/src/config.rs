use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub ldap: LdapConfig,
    pub jwt: JwtConfig,
    pub crypto: CryptoConfig,
    pub storage: StorageConfig,
    #[serde(default)]
    pub smtp: SmtpConfig,
    #[serde(default)]
    pub stripe: StripeConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    /// Allowed CORS origins (e.g. ["https://console.example.com"]).
    /// If empty, all origins are allowed (dev mode only — logs a warning).
    #[serde(default)]
    pub cors_origins: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LdapConfig {
    /// ldap(s):// URL used for bind-based authentication
    pub url: String,
    pub base_dn: String,
    pub bind_dn: String,
    pub bind_password: String,
    pub user_ou: String,
    pub group_ou: String,
    pub user_filter: String,
    pub sync_interval_secs: u64,
    pub connection_timeout_secs: u64,
    pub pool_size: usize,
    /// HTTP base URL of the LLDAP web/API server, e.g. http://127.0.0.1:17170
    /// Used for GraphQL admin operations (create user, change password, etc.)
    pub http_url: String,
    /// LLDAP admin username (plain, not a DN)
    #[serde(default = "default_admin_username")]
    pub admin_username: String,
    /// Optional group ID to assign new self-registered users
    pub default_user_group_id: Option<i64>,
}

fn default_admin_username() -> String {
    "admin".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct JwtConfig {
    /// Access token lifetime (short). Default 2 hours.
    #[serde(default = "default_access_expiry")]
    pub expiry_hours: i64,
    /// Refresh token lifetime (long). Default 7 days (168 hours).
    #[serde(default = "default_refresh_expiry")]
    pub refresh_expiry_hours: i64,
}

fn default_access_expiry() -> i64 { 2 }
fn default_refresh_expiry() -> i64 { 168 }

#[derive(Debug, Clone, Deserialize)]
pub struct CryptoConfig {
    pub key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub root_path: String,
}

/// Optional SMTP configuration. If `host` is empty the mailer is disabled
/// and password-reset emails are only logged (useful in dev).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SmtpConfig {
    #[serde(default)]
    pub host: String,
    #[serde(default = "default_smtp_port")]
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    /// Display name + address, e.g. "QuickStack <noreply@example.com>"
    #[serde(default)]
    pub from: String,
    /// Use STARTTLS (true) vs implicit TLS (false)
    #[serde(default = "default_true")]
    pub starttls: bool,
}

fn default_smtp_port() -> u16 {
    587
}
fn default_true() -> bool {
    true
}

/// Stripe payment gateway configuration.
/// Leave `secret_key` empty to disable Stripe integration entirely.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct StripeConfig {
    /// Stripe secret key (sk_test_... or sk_live_...)
    #[serde(default)]
    pub secret_key: String,
    /// Stripe webhook signing secret (whsec_...)
    #[serde(default)]
    pub webhook_secret: String,
    /// Currency code for Checkout sessions. Only "cny" and "usd" are accepted.
    #[serde(default = "default_currency")]
    pub currency: String,
    /// Predefined top-up amounts (in the currency's smallest unit, e.g. cents/分).
    /// If empty, users specify a custom amount.
    #[serde(default = "default_topup_amounts")]
    pub topup_amounts: Vec<i64>,
    /// URL the user returns to after a successful Checkout session.
    /// `{session_id}` is replaced with the Stripe session ID.
    #[serde(default = "default_success_url")]
    pub success_url: String,
    /// URL the user returns to if they cancel Checkout.
    #[serde(default = "default_cancel_url")]
    pub cancel_url: String,
}

fn default_currency() -> String {
    "cny".to_string()
}
fn default_topup_amounts() -> Vec<i64> {
    vec![1000, 5000, 10000, 50000]
}
fn default_success_url() -> String {
    "http://localhost:3000/billing?payment=success&session_id={session_id}".to_string()
}
fn default_cancel_url() -> String {
    "http://localhost:3000/billing?payment=cancelled".to_string()
}

impl StripeConfig {
    pub fn is_enabled(&self) -> bool {
        !self.secret_key.is_empty()
    }
}

impl SmtpConfig {
    pub fn is_configured(&self) -> bool {
        !self.host.is_empty() && !self.from.is_empty()
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let cfg = config::Config::builder()
            .add_source(config::File::with_name("config/default").required(false))
            .add_source(
                config::Environment::default()
                    .separator("__")
                    .try_parsing(true),
            )
            .build()?;

        Ok(cfg.try_deserialize()?)
    }
}
