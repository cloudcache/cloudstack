-- Subscription plans + user subscriptions (corrected design).
--
-- Quota lives on subscription_plans → allocated to projects (not to users).
-- Users have NO quota columns.
-- A default project is auto-created in application code when a subscription
-- first becomes ACTIVE; that project receives the full plan quota.
--
-- All DDL is written to be fully idempotent on MySQL 5.7+:
--   DROP COLUMN  → only if column exists   (via information_schema + PREPARE)
--   ADD COLUMN   → only if column missing   (via information_schema + PREPARE)
--   CREATE TABLE → IF NOT EXISTS            (native)
--   INSERT       → INSERT IGNORE            (native)

-- ─── Remove quota columns from users ─────────────────────────────────────────

SET @_drop_cols = (
  SELECT GROUP_CONCAT('DROP COLUMN `', COLUMN_NAME, '`' ORDER BY COLUMN_NAME SEPARATOR ', ')
  FROM information_schema.COLUMNS
  WHERE TABLE_SCHEMA = DATABASE()
    AND TABLE_NAME   = 'users'
    AND COLUMN_NAME  IN ('quota_cpu_mcores','quota_mem_mb','quota_storage_gb',
                         'quota_apps','quota_db_instances')
);
SET @_sql = IF(
  @_drop_cols IS NOT NULL AND @_drop_cols != '',
  CONCAT('ALTER TABLE users ', @_drop_cols),
  'SELECT 1 /* quota columns already removed */'
);
PREPARE _stmt FROM @_sql;
EXECUTE _stmt;
DEALLOCATE PREPARE _stmt;

-- ─── Extend projects: default flag + missing quota dimensions ─────────────────

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='projects' AND COLUMN_NAME='is_default') = 0,
  'ALTER TABLE projects ADD COLUMN `is_default` TINYINT(1) NOT NULL DEFAULT 0 COMMENT "订阅激活时自动创建的默认项目" AFTER `is_active`',
  'SELECT 1');
PREPARE _stmt FROM @_sql;
EXECUTE _stmt;
DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='projects' AND COLUMN_NAME='quota_bandwidth_gb') = 0,
  'ALTER TABLE projects ADD COLUMN `quota_bandwidth_gb` INT UNSIGNED NOT NULL DEFAULT 0 COMMENT "月出流量上限 GB" AFTER `quota_db_instances`',
  'SELECT 1');
PREPARE _stmt FROM @_sql;
EXECUTE _stmt;
DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='projects' AND COLUMN_NAME='quota_domain_count') = 0,
  'ALTER TABLE projects ADD COLUMN `quota_domain_count` INT UNSIGNED NOT NULL DEFAULT 0 COMMENT "域名总数上限" AFTER `quota_bandwidth_gb`',
  'SELECT 1');
PREPARE _stmt FROM @_sql;
EXECUTE _stmt;
DEALLOCATE PREPARE _stmt;

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='projects' AND COLUMN_NAME='quota_request_million') = 0,
  'ALTER TABLE projects ADD COLUMN `quota_request_million` INT UNSIGNED NOT NULL DEFAULT 0 COMMENT "月请求数上限(百万)" AFTER `quota_domain_count`',
  'SELECT 1');
PREPARE _stmt FROM @_sql;
EXECUTE _stmt;
DEALLOCATE PREPARE _stmt;

-- ─── Subscription plans ───────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS subscription_plans (
  id                       CHAR(36)       NOT NULL,
  name                     VARCHAR(64)    NOT NULL  COMMENT '内部 slug',
  display_name             VARCHAR(128)   NOT NULL,
  description              TEXT           NULL,

  price_monthly            DECIMAL(10,2)  NOT NULL  DEFAULT 0.00
                           COMMENT '月费 (0 = 免费)',
  price_annually           DECIMAL(10,2)  NULL
                           COMMENT '年费折扣价 NULL=不支持年付',

  -- 9 quota dimensions; 0 = unlimited
  quota_cpu_mcores         INT UNSIGNED   NOT NULL  DEFAULT 0,
  quota_mem_mb             INT UNSIGNED   NOT NULL  DEFAULT 0,
  quota_storage_gb         INT UNSIGNED   NOT NULL  DEFAULT 0,
  quota_bandwidth_gb       INT UNSIGNED   NOT NULL  DEFAULT 0
                           COMMENT '月出流量 GB',
  quota_domain_count       INT UNSIGNED   NOT NULL  DEFAULT 0
                           COMMENT '域名总数',
  quota_db_instance_count  INT UNSIGNED   NOT NULL  DEFAULT 0
                           COMMENT '数据库实例数',
  quota_project_count      INT UNSIGNED   NOT NULL  DEFAULT 0
                           COMMENT '最多可创建项目数 (0=不限)',
  quota_app_count          INT UNSIGNED   NOT NULL  DEFAULT 0
                           COMMENT '应用实例总数',
  quota_request_million    INT UNSIGNED   NOT NULL  DEFAULT 0
                           COMMENT '月请求数(百万)',

  is_active                TINYINT(1)     NOT NULL  DEFAULT 1,
  is_public                TINYINT(1)     NOT NULL  DEFAULT 1
                           COMMENT '用户可见并自助订阅',
  sort_order               SMALLINT       NOT NULL  DEFAULT 0
                           COMMENT '前端显示顺序',

  created_at               DATETIME       NOT NULL  DEFAULT CURRENT_TIMESTAMP,
  updated_at               DATETIME       NOT NULL  DEFAULT CURRENT_TIMESTAMP
                                          ON UPDATE CURRENT_TIMESTAMP,

  PRIMARY KEY (id),
  UNIQUE KEY uq_plan_name (name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Seed a default free plan
INSERT IGNORE INTO subscription_plans
  (id, name, display_name, description, price_monthly,
   quota_cpu_mcores, quota_mem_mb, quota_storage_gb,
   quota_bandwidth_gb, quota_domain_count, quota_db_instance_count,
   quota_project_count, quota_app_count, quota_request_million,
   is_active, is_public, sort_order)
VALUES
  (UUID(), 'free', '免费版', '适合个人试用', 0.00,
   2000, 2048, 10,
   50, 3, 2,
   2, 5, 1,
   1, 1, 0);

-- ─── User subscriptions ───────────────────────────────────────────────────────

CREATE TABLE IF NOT EXISTS user_subscriptions (
  id              CHAR(36)      NOT NULL,
  user_id         CHAR(36)      NOT NULL,
  plan_id         CHAR(36)      NOT NULL,

  status          ENUM('PENDING','ACTIVE','OVERDUE','EXPIRED','CANCELLED')
                  NOT NULL  DEFAULT 'PENDING',
  billing_cycle   ENUM('MONTHLY','ANNUALLY','LIFETIME','CUSTOM')
                  NOT NULL  DEFAULT 'MONTHLY',

  started_at      DATETIME      NULL,
  expires_at      DATETIME      NULL    COMMENT 'NULL = 永不过期',
  auto_renew      TINYINT(1)    NOT NULL DEFAULT 1,

  cancelled_at    DATETIME      NULL,
  cancel_reason   VARCHAR(512)  NULL,

  price_paid      DECIMAL(10,2) NOT NULL DEFAULT 0.00,
  created_by      CHAR(36)      NULL    COMMENT 'admin user_id, NULL 表示用户自订',

  created_at      DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at      DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP
                                ON UPDATE CURRENT_TIMESTAMP,

  PRIMARY KEY (id),
  KEY idx_sub_user   (user_id),
  KEY idx_sub_plan   (plan_id),
  KEY idx_sub_status (status, expires_at),
  CONSTRAINT fk_sub_user       FOREIGN KEY (user_id)    REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT fk_sub_plan       FOREIGN KEY (plan_id)    REFERENCES subscription_plans(id),
  CONSTRAINT fk_sub_created_by FOREIGN KEY (created_by) REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Platform config keys for subscription behaviour
INSERT IGNORE INTO platform_config (`key`, `value`, description) VALUES
  ('subscription_self_service',    '1',    '是否允许用户自助订阅/升降级'),
  ('subscription_expiry_action',   'KEEP', '到期后配额处理: KEEP 保留 | RESET 清零'),
  ('subscription_allow_downgrade', '1',    '是否允许用户降级计划');
