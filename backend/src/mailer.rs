/// Simple SMTP mailer wrapper around lettre.
///
/// If SMTP is not configured (`smtp.host` empty) every send call logs the
/// email body to tracing instead — handy during development.
use lettre::{
    message::header::ContentType, transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};

use crate::{config::SmtpConfig, error::{AppError, AppResult}};

pub struct Mailer {
    from: String,
    transport: Option<AsyncSmtpTransport<Tokio1Executor>>,
}

impl Mailer {
    pub fn new(cfg: &SmtpConfig) -> AppResult<Self> {
        if !cfg.is_configured() {
            tracing::warn!("SMTP not configured — emails will be logged only");
            return Ok(Self { from: cfg.from.clone(), transport: None });
        }

        let creds = Credentials::new(cfg.username.clone(), cfg.password.clone());

        let transport = if cfg.starttls {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host)
                .map_err(|e| AppError::Internal(format!("smtp starttls: {e}")))?
                .port(cfg.port)
                .credentials(creds)
                .build()
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.host)
                .map_err(|e| AppError::Internal(format!("smtp relay: {e}")))?
                .port(cfg.port)
                .credentials(creds)
                .build()
        };

        Ok(Self {
            from: cfg.from.clone(),
            transport: Some(transport),
        })
    }

    pub async fn send(&self, to: &str, subject: &str, html_body: &str) -> AppResult<()> {
        let Some(transport) = &self.transport else {
            tracing::info!("[DEV EMAIL] To: {to}\nSubject: {subject}\n{html_body}");
            return Ok(());
        };

        let email = Message::builder()
            .from(
                self.from
                    .parse()
                    .map_err(|e| AppError::Internal(format!("smtp from addr: {e}")))?,
            )
            .to(to
                .parse()
                .map_err(|e| AppError::Internal(format!("smtp to addr: {e}")))?)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(html_body.to_string())
            .map_err(|e| AppError::Internal(format!("smtp build: {e}")))?;

        transport
            .send(email)
            .await
            .map_err(|e| AppError::Internal(format!("smtp send: {e}")))?;

        Ok(())
    }

    // ─── Template helpers ─────────────────────────────────────────────────────

    pub async fn send_password_reset(
        &self,
        to_email: &str,
        username: &str,
        reset_url: &str,
        platform_name: &str,
    ) -> AppResult<()> {
        let subject = format!("{platform_name} — 密码重置");
        let body = format!(
            r#"<!DOCTYPE html><html><body style="font-family:sans-serif;max-width:600px;margin:auto">
<h2>{platform_name} 密码重置</h2>
<p>您好 <strong>{username}</strong>，</p>
<p>我们收到了您的密码重置请求。点击下方按钮设置新密码（链接 <strong>1 小时</strong>内有效）：</p>
<p><a href="{reset_url}" style="display:inline-block;padding:12px 24px;background:#2563eb;color:#fff;text-decoration:none;border-radius:6px">重置密码</a></p>
<p style="color:#6b7280;font-size:13px">如果您没有发起此请求，请忽略本邮件。</p>
<hr><p style="color:#9ca3af;font-size:12px">{platform_name}</p>
</body></html>"#
        );
        self.send(to_email, &subject, &body).await
    }

    pub async fn send_email_verification(
        &self,
        to_email: &str,
        username: &str,
        verify_url: &str,
        platform_name: &str,
    ) -> AppResult<()> {
        let subject = format!("{platform_name} — 邮箱验证");
        let body = format!(
            r#"<!DOCTYPE html><html><body style="font-family:sans-serif;max-width:600px;margin:auto">
<h2>{platform_name} 邮箱验证</h2>
<p>您好 <strong>{username}</strong>，</p>
<p>欢迎注册 {platform_name}。请点击下方按钮完成邮箱验证（链接 <strong>24 小时</strong>内有效）：</p>
<p><a href="{verify_url}" style="display:inline-block;padding:12px 24px;background:#2563eb;color:#fff;text-decoration:none;border-radius:6px">验证邮箱</a></p>
<p style="color:#6b7280;font-size:13px">如果按钮无法点击，请复制以下链接到浏览器：<br><code>{verify_url}</code></p>
<p style="color:#6b7280;font-size:13px">如果您没有注册账号，请忽略本邮件。</p>
<hr><p style="color:#9ca3af;font-size:12px">{platform_name}</p>
</body></html>"#
        );
        self.send(to_email, &subject, &body).await
    }

    pub async fn send_overdue_notice(
        &self,
        to_email: &str,
        username: &str,
        fee_amount: f64,
        new_balance: f64,
        currency: &str,
        platform_name: &str,
    ) -> AppResult<()> {
        let cur = currency.to_uppercase();
        let subject = format!("{platform_name} — 账户余额不足提醒");
        let body = format!(
            r#"<!DOCTYPE html><html><body style="font-family:sans-serif;max-width:600px;margin:auto">
<h2>{platform_name} 余额不足提醒</h2>
<p>您好 <strong>{username}</strong>，</p>
<p>您的账户余额已为负，今日产生逾期费用 <strong>{fee_amount:.2} {cur}</strong>，当前余额为 <strong>{new_balance:.2} {cur}</strong>。</p>
<p>为避免逾期费用继续累积、相关应用被暂停，请尽快充值。</p>
<hr><p style="color:#9ca3af;font-size:12px">{platform_name}</p>
</body></html>"#
        );
        self.send(to_email, &subject, &body).await
    }

    pub async fn send_registration_success(
        &self,
        to_email: &str,
        username: &str,
        login_url: &str,
        platform_name: &str,
    ) -> AppResult<()> {
        let subject = format!("欢迎加入 {platform_name}");
        let body = format!(
            r#"<!DOCTYPE html><html><body style="font-family:sans-serif;max-width:600px;margin:auto">
<h2>欢迎使用 {platform_name}</h2>
<p>您好 <strong>{username}</strong>，您的账号已成功创建。</p>
<p><a href="{login_url}" style="display:inline-block;padding:12px 24px;background:#2563eb;color:#fff;text-decoration:none;border-radius:6px">立即登录</a></p>
<hr><p style="color:#9ca3af;font-size:12px">{platform_name}</p>
</body></html>"#
        );
        self.send(to_email, &subject, &body).await
    }
}
