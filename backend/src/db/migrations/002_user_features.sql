-- User feature extensions: SSH keys, password reset, wallet/billing, usage

-- ------------------------------------------------------------
-- SSH public keys
-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS user_ssh_keys (
  id           CHAR(36)      NOT NULL,
  user_id      CHAR(36)      NOT NULL,
  name         VARCHAR(128)  NOT NULL  COMMENT '用户自定义标签',
  public_key   TEXT          NOT NULL,
  fingerprint  VARCHAR(256)  NOT NULL  COMMENT 'SHA256:base64 (key-type)',
  created_at   DATETIME      NOT NULL  DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uq_ssh_fp_user (user_id, fingerprint),
  KEY idx_ssh_keys_user (user_id),
  CONSTRAINT fk_ssh_keys_user FOREIGN KEY (user_id)
    REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------
-- Password reset tokens (for forgot-password flow)
-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS password_reset_tokens (
  id          CHAR(36)      NOT NULL,
  user_id     CHAR(36)      NOT NULL,
  token_hash  VARCHAR(256)  NOT NULL  COMMENT 'SHA-256(raw token)',
  expires_at  DATETIME      NOT NULL,
  used_at     DATETIME      NULL,
  created_at  DATETIME      NOT NULL  DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uq_reset_token (token_hash),
  KEY idx_reset_token_user (user_id),
  CONSTRAINT fk_reset_token_user FOREIGN KEY (user_id)
    REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------
-- Wallets (one per user, created on first login)
-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS user_wallets (
  user_id     CHAR(36)       NOT NULL,
  balance     DECIMAL(12,2)  NOT NULL  DEFAULT 0.00,
  currency    VARCHAR(8)     NOT NULL  DEFAULT 'CNY',
  updated_at  DATETIME       NOT NULL  DEFAULT CURRENT_TIMESTAMP
                                       ON UPDATE CURRENT_TIMESTAMP,
  PRIMARY KEY (user_id),
  CONSTRAINT fk_wallet_user FOREIGN KEY (user_id)
    REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------
-- Wallet transactions
-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS wallet_transactions (
  id             CHAR(36)                                          NOT NULL,
  user_id        CHAR(36)                                          NOT NULL,
  tx_type        ENUM('RECHARGE','DEDUCTION','REFUND','ADJUSTMENT') NOT NULL,
  amount         DECIMAL(12,2)                                     NOT NULL
                 COMMENT '正数=充入, 负数=扣减',
  balance_after  DECIMAL(12,2)                                     NOT NULL,
  description    VARCHAR(512)                                      NULL,
  ref_id         VARCHAR(256)                                      NULL
                 COMMENT '关联 invoice_id 或 snapshot_id',
  operator_id    CHAR(36)                                          NULL
                 COMMENT '操作管理员 user_id',
  created_at     DATETIME                                          NOT NULL
                 DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  KEY idx_wallet_tx_user    (user_id),
  KEY idx_wallet_tx_type    (tx_type),
  KEY idx_wallet_tx_created (created_at),
  CONSTRAINT fk_wallet_tx_user FOREIGN KEY (user_id)
    REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT fk_wallet_tx_op FOREIGN KEY (operator_id)
    REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------
-- Hourly usage snapshots (inserted by billing cron job)
-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS usage_snapshots (
  id               CHAR(36)          NOT NULL,
  user_id          CHAR(36)          NOT NULL,
  snapshot_time    DATETIME          NOT NULL  COMMENT '整点时间',
  app_count        SMALLINT UNSIGNED NOT NULL  DEFAULT 0,
  db_count         SMALLINT UNSIGNED NOT NULL  DEFAULT 0,
  cpu_mcores_used  INT UNSIGNED      NOT NULL  DEFAULT 0,
  mem_mb_used      INT UNSIGNED      NOT NULL  DEFAULT 0,
  storage_gb_used  INT UNSIGNED      NOT NULL  DEFAULT 0,
  cost             DECIMAL(10,4)     NOT NULL  DEFAULT 0.0000
                   COMMENT '本小时费用',
  created_at       DATETIME          NOT NULL  DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uq_usage_user_hour (user_id, snapshot_time),
  KEY idx_usage_user_time (user_id, snapshot_time),
  CONSTRAINT fk_usage_user FOREIGN KEY (user_id)
    REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------
-- Monthly invoices
-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS invoices (
  id            CHAR(36)                              NOT NULL,
  user_id       CHAR(36)                              NOT NULL,
  invoice_no    VARCHAR(64)                           NOT NULL  COMMENT '账单编号',
  period_start  DATE                                  NOT NULL,
  period_end    DATE                                  NOT NULL,
  total_amount  DECIMAL(12,2)                         NOT NULL  DEFAULT 0.00,
  status        ENUM('DRAFT','ISSUED','PAID','VOID')  NOT NULL  DEFAULT 'DRAFT',
  items         JSON                                  NULL      COMMENT '明细行',
  created_at    DATETIME                              NOT NULL  DEFAULT CURRENT_TIMESTAMP,
  issued_at     DATETIME                              NULL,
  paid_at       DATETIME                              NULL,
  PRIMARY KEY (id),
  UNIQUE KEY uq_invoice_no (invoice_no),
  KEY idx_invoices_user   (user_id),
  KEY idx_invoices_status (status),
  CONSTRAINT fk_invoices_user FOREIGN KEY (user_id)
    REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Default pricing config keys (admin adjusts via platform-config API)
INSERT IGNORE INTO platform_config (`key`, `value`, description) VALUES
  ('billing_enabled',           '0',      '是否启用计费'),
  ('billing_currency',          'CNY',    '计费货币单位'),
  ('price_cpu_mcores_hour',     '0.0001', '每 mCore·小时 价格'),
  ('price_mem_mb_hour',         '0.0001', '每 MB·小时 价格'),
  ('price_storage_gb_month',    '0.10',   '每 GB·月 价格'),
  ('registration_enabled',      '1',      '是否开放用户自主注册'),
  ('registration_require_approval', '0',  '注册是否需要管理员审批'),
  ('allow_user_create_projects',    '1',  '是否允许普通用户自主创建项目'),
  ('frontend_url',                  '',   '前端访问地址，用于邮件中的链接');
