-- 009_network_billing.sql
-- LB bandwidth sampling, monthly P95 network charges, overdue charge records

-- 5-minute LB bandwidth samples collected from Pingora per app
CREATE TABLE IF NOT EXISTS lb_bandwidth_samples (
  id               CHAR(36)   NOT NULL PRIMARY KEY,
  app_id           CHAR(36)   NOT NULL,
  project_id       CHAR(36)   NOT NULL,
  sampled_at       DATETIME   NOT NULL,  -- truncated to 5-min boundary
  duration_secs    SMALLINT   NOT NULL DEFAULT 300,
  ingress_bytes    BIGINT     NOT NULL DEFAULT 0,
  egress_bytes     BIGINT     NOT NULL DEFAULT 0,
  req_count        INT        NOT NULL DEFAULT 0,
  req_body_bytes   BIGINT     NOT NULL DEFAULT 0,
  resp_body_bytes  BIGINT     NOT NULL DEFAULT 0,

  UNIQUE KEY uq_lbs_app_time (app_id, sampled_at),
  KEY idx_lbs_project_time (project_id, sampled_at),
  CONSTRAINT fk_lbs_app FOREIGN KEY (app_id) REFERENCES apps(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Monthly computed network charges per project
CREATE TABLE IF NOT EXISTS monthly_network_charges (
  id                   CHAR(36)      NOT NULL PRIMARY KEY,
  project_id           CHAR(36)      NOT NULL,
  user_id              CHAR(36)      NOT NULL,
  billing_month        CHAR(7)       NOT NULL,  -- 'YYYY-MM'
  p95_egress_mbps      DECIMAL(12,4) NOT NULL DEFAULT 0,
  total_egress_bytes   BIGINT        NOT NULL DEFAULT 0,
  total_ingress_bytes  BIGINT        NOT NULL DEFAULT 0,
  mean_req_body_bytes  BIGINT        NOT NULL DEFAULT 0,
  mean_resp_body_bytes BIGINT        NOT NULL DEFAULT 0,
  total_req_count      BIGINT        NOT NULL DEFAULT 0,
  egress_charge        DECIMAL(10,4) NOT NULL DEFAULT 0,
  status               ENUM('PENDING','CHARGED','WAIVED') NOT NULL DEFAULT 'PENDING',
  created_at           DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP,
  charged_at           DATETIME      NULL,

  UNIQUE KEY uq_mnc_project_month (project_id, billing_month),
  KEY idx_mnc_user (user_id),
  CONSTRAINT fk_mnc_project FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Daily overdue fee records (applied when wallet balance < 0)
CREATE TABLE IF NOT EXISTS overdue_charges (
  id              CHAR(36)      NOT NULL PRIMARY KEY,
  user_id         CHAR(36)      NOT NULL,
  charge_date     DATE          NOT NULL,
  overdue_balance DECIMAL(12,2) NOT NULL,  -- negative balance at time of charge
  fee_pct         DECIMAL(6,4)  NOT NULL,  -- e.g. 0.0050 = 0.50%/day
  fee_amount      DECIMAL(12,4) NOT NULL,
  status          ENUM('PENDING','APPLIED','WAIVED') NOT NULL DEFAULT 'PENDING',
  created_at      DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP,

  UNIQUE KEY uq_oc_user_date (user_id, charge_date),
  CONSTRAINT fk_oc_user FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Pricing & overdue config
INSERT IGNORE INTO platform_config (`key`, `value`, description) VALUES
  ('price_egress_p95_mbps_month', '0',    'Monthly P95 egress price (CNY/Mbps). 0 = disabled, use price_egress_gb instead'),
  ('price_egress_gb',             '0.08', 'Egress traffic price per GB (CNY). Used when P95 billing disabled'),
  ('price_ingress_gb',            '0',    'Ingress traffic price per GB (CNY). 0 = free'),
  ('price_req_body_gb',           '0',    'Request body traffic price per GB (CNY). 0 = free'),
  ('billing_overdue_grace_days',  '3',    'Days after balance goes negative before daily fee starts'),
  ('billing_overdue_daily_fee_pct','0.05','Daily overdue fee as % of outstanding negative balance');
