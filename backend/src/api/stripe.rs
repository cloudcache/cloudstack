/// Stripe Checkout integration for wallet top-up.
///
/// Flow:
///   1. User calls POST /api/v1/billing/topup → creates Stripe Checkout session
///   2. User is redirected to Stripe-hosted payment page
///   3. Stripe sends webhook POST /stripe/webhook → checkout.session.completed
///   4. Webhook handler credits the user's wallet and records the transaction
///
/// All amounts are in the currency's smallest unit (分 for CNY, cents for USD).
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use serde::Deserialize;
use stripe::{
    CheckoutSession, CheckoutSessionMode, Client as StripeClient, CreateCheckoutSession,
    CreateCheckoutSessionLineItems, CreateCheckoutSessionLineItemsPriceData,
    CreateCheckoutSessionLineItemsPriceDataProductData, Currency, EventObject, EventType, Webhook,
};
use uuid::Uuid;

use crate::{
    auth::middleware::AuthUser,
    error::{AppError, AppResult},
    state::AppState,
};

// ─── Config check ────────────────────────────────────────────────────────────

fn require_stripe(state: &AppState) -> AppResult<StripeClient> {
    if !state.config.stripe.is_enabled() {
        return Err(AppError::BadRequest(
            "stripe payments are not configured".into(),
        ));
    }
    Ok(StripeClient::new(&state.config.stripe.secret_key))
}

fn parse_currency(s: &str) -> Result<Currency, AppError> {
    match s.to_lowercase().as_str() {
        "cny" => Ok(Currency::CNY),
        "usd" => Ok(Currency::USD),
        _ => Err(AppError::BadRequest(format!(
            "Unsupported currency '{}'. Only CNY and USD are accepted.",
            s
        ))),
    }
}

// ─── GET /api/v1/billing/topup/config ────────────────────────────────────────

/// Returns Stripe-related client config (no secrets).
pub async fn topup_config(State(state): State<AppState>) -> AppResult<impl IntoResponse> {
    Ok(Json(serde_json::json!({
        "enabled": state.config.stripe.is_enabled(),
        "currency": state.config.stripe.currency,
        "topup_amounts": state.config.stripe.topup_amounts,
    })))
}

// ─── POST /api/v1/billing/topup ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TopupRequest {
    /// Amount in smallest currency unit (分/cents)
    pub amount: i64,
}

/// Create a Stripe Checkout Session for wallet top-up.
pub async fn create_topup(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
    Json(body): Json<TopupRequest>,
) -> AppResult<impl IntoResponse> {
    let client = require_stripe(&state)?;

    if body.amount <= 0 {
        return Err(AppError::BadRequest("amount must be positive".into()));
    }

    let cfg = &state.config.stripe;
    let currency = parse_currency(&cfg.currency)?;

    // Format display amount (divide by 100 for most currencies)
    let display_amount = body.amount as f64 / 100.0;
    let product_name = format!(
        "Wallet Top-up ({:.2} {})",
        display_amount,
        cfg.currency.to_uppercase()
    );

    let success_url = cfg
        .success_url
        .replace("{session_id}", "{CHECKOUT_SESSION_ID}");
    let cancel_url = cfg.cancel_url.clone();

    // Create Checkout Session via async-stripe
    let mut params = CreateCheckoutSession::new();
    params.mode = Some(CheckoutSessionMode::Payment);
    params.success_url = Some(&success_url);
    params.cancel_url = Some(&cancel_url);
    params.client_reference_id = Some(&auth.user_id);
    params.line_items = Some(vec![CreateCheckoutSessionLineItems {
        quantity: Some(1),
        price_data: Some(CreateCheckoutSessionLineItemsPriceData {
            currency,
            unit_amount: Some(body.amount),
            product_data: Some(CreateCheckoutSessionLineItemsPriceDataProductData {
                name: product_name,
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    }]);
    params.metadata = Some(
        [
            ("user_id".to_string(), auth.user_id.clone()),
            ("purpose".to_string(), "wallet_topup".to_string()),
        ]
        .into(),
    );

    let session = CheckoutSession::create(&client, params)
        .await
        .map_err(|e| AppError::Internal(format!("stripe: {e}")))?;

    let session_id = session.id.to_string();
    let checkout_url = session
        .url
        .ok_or_else(|| AppError::Internal("stripe: no checkout URL returned".into()))?;

    // Record pending payment
    let id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO stripe_payments
             (id, user_id, stripe_session_id, amount, currency, status)
           VALUES (?, ?, ?, ?, ?, 'PENDING')"#,
        id,
        auth.user_id,
        session_id,
        body.amount,
        cfg.currency,
    )
    .execute(&state.db)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "checkout_url": checkout_url,
            "session_id": session_id,
        })),
    ))
}

// ─── GET /api/v1/billing/topup/history ───────────────────────────────────────

pub async fn topup_history(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthUser>,
) -> AppResult<impl IntoResponse> {
    let rows = sqlx::query!(
        r#"SELECT id, stripe_session_id, amount, currency, status, created_at, completed_at
           FROM stripe_payments
           WHERE user_id = ?
           ORDER BY created_at DESC
           LIMIT 50"#,
        auth.user_id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "session_id": r.stripe_session_id,
                    "amount": r.amount,
                    "currency": r.currency,
                    "status": r.status,
                    "created_at": r.created_at,
                    "completed_at": r.completed_at,
                })
            })
            .collect::<Vec<_>>(),
    ))
}

// ─── POST /stripe/webhook ────────────────────────────────────────────────────
// This is a PUBLIC endpoint — no auth middleware. Verified via Stripe signature.

pub async fn stripe_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<impl IntoResponse> {
    let cfg = &state.config.stripe;
    if !cfg.is_enabled() {
        return Err(AppError::BadRequest("stripe not configured".into()));
    }

    // Verify signature
    let sig = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::BadRequest("missing stripe-signature header".into()))?;

    let payload = std::str::from_utf8(&body)
        .map_err(|_| AppError::BadRequest("invalid UTF-8 payload".into()))?;

    let event = Webhook::construct_event(payload, sig, &cfg.webhook_secret)
        .map_err(|e| AppError::BadRequest(format!("webhook signature invalid: {e}")))?;

    match event.type_ {
        EventType::CheckoutSessionCompleted => {
            if let EventObject::CheckoutSession(session) = event.data.object {
                handle_checkout_completed(&state, &session).await?;
            }
        }
        _ => {
            tracing::debug!(event_type = ?event.type_, "ignoring stripe event");
        }
    }

    Ok(StatusCode::OK)
}

async fn handle_checkout_completed(state: &AppState, session: &CheckoutSession) -> AppResult<()> {
    let session_id = session.id.to_string();

    // Find our pending payment record
    let payment = sqlx::query!(
        r#"SELECT id, user_id, amount, currency, status
           FROM stripe_payments
           WHERE stripe_session_id = ?"#,
        session_id,
    )
    .fetch_optional(&state.db)
    .await?;

    let Some(payment) = payment else {
        tracing::warn!(session_id, "stripe webhook: no matching payment record");
        return Ok(());
    };

    if payment.status != "PENDING" {
        tracing::debug!(session_id, status = %payment.status, "stripe webhook: payment already processed");
        return Ok(());
    }

    let payment_intent = session
        .payment_intent
        .as_ref()
        .map(|pi| pi.id().to_string());

    // Convert smallest-unit amount to decimal (e.g. 1000分 → 10.00)
    let decimal_amount = rust_decimal::Decimal::new(payment.amount, 2);

    // Atomic: wallet credit + transaction log + payment record update
    let mut db_tx = state.db.begin().await?;

    // Credit wallet
    sqlx::query!(
        r#"INSERT INTO user_wallets (user_id, balance) VALUES (?, ?)
           ON DUPLICATE KEY UPDATE balance = balance + VALUES(balance)"#,
        payment.user_id,
        decimal_amount,
    )
    .execute(&mut *db_tx)
    .await?;

    let new_balance = sqlx::query_scalar!(
        r#"SELECT balance FROM user_wallets WHERE user_id = ?"#,
        payment.user_id,
    )
    .fetch_one(&mut *db_tx)
    .await?;

    // Record wallet transaction
    let tx_id = Uuid::new_v4().to_string();
    sqlx::query!(
        r#"INSERT INTO wallet_transactions
             (id, user_id, tx_type, amount, balance_after, description, ref_id)
           VALUES (?, ?, 'RECHARGE', ?, ?, 'Stripe top-up', ?)"#,
        tx_id,
        payment.user_id,
        decimal_amount,
        new_balance,
        payment.id,
    )
    .execute(&mut *db_tx)
    .await?;

    // Update payment record
    sqlx::query!(
        r#"UPDATE stripe_payments
           SET status = 'COMPLETED',
               stripe_payment_intent = ?,
               wallet_tx_id = ?,
               completed_at = NOW()
           WHERE id = ?"#,
        payment_intent,
        tx_id,
        payment.id,
    )
    .execute(&mut *db_tx)
    .await?;

    db_tx.commit().await?;

    tracing::info!(
        user_id = %payment.user_id,
        amount = %decimal_amount,
        session_id,
        "stripe top-up credited to wallet"
    );

    Ok(())
}
