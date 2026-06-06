-- ── Refresh tokens for dual-token JWT auth ───────────────────────────────────
-- Short-lived access token (2h) + long-lived refresh token (7d) with rotation.
-- Each refresh invalidates the old token and issues a new pair.

CREATE TABLE IF NOT EXISTS refresh_tokens (
    id            CHAR(36)     NOT NULL PRIMARY KEY,
    user_id       CHAR(36)     NOT NULL,
    token_hash    CHAR(64)     NOT NULL              COMMENT 'SHA-256 hex of the raw refresh token',
    expires_at    DATETIME     NOT NULL,
    created_at    DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE  INDEX idx_rt_hash (token_hash),
    INDEX   idx_rt_user (user_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
