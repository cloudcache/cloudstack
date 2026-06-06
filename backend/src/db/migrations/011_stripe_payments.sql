-- Stripe payment records: tracks Checkout sessions and maps them to wallet top-ups.

CREATE TABLE IF NOT EXISTS stripe_payments (
    id              VARCHAR(36)  NOT NULL PRIMARY KEY,
    user_id         VARCHAR(36)  NOT NULL,
    -- Stripe Checkout session ID (cs_...)
    stripe_session_id VARCHAR(255) NOT NULL,
    -- Stripe Payment Intent ID (pi_...), populated after webhook
    stripe_payment_intent VARCHAR(255) DEFAULT NULL,
    -- Amount in smallest currency unit (cents/分)
    amount          BIGINT       NOT NULL,
    currency        VARCHAR(8)   NOT NULL DEFAULT 'cny',
    -- PENDING → COMPLETED / EXPIRED / FAILED
    status          VARCHAR(20)  NOT NULL DEFAULT 'PENDING',
    -- wallet_transactions.id — set when wallet is credited
    wallet_tx_id    VARCHAR(36)  DEFAULT NULL,
    created_at      DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at    DATETIME     DEFAULT NULL,
    UNIQUE KEY uq_stripe_session (stripe_session_id),
    KEY idx_stripe_user (user_id),
    KEY idx_stripe_status (status)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
