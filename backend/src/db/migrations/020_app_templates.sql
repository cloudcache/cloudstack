-- 020_app_templates.sql
-- Move app templates from hardcoded TS files into the database, with
-- visibility tiers (PUBLIC / ORG / PRIVATE) so both admins and tenants can
-- maintain their own template catalog. The `requirements` and `bindings`
-- columns are reserved for Phase 2 (managed-service binding); for now they
-- default to empty arrays so the existing template flow keeps working.

SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.TABLES
   WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'app_templates') = 0,
  'CREATE TABLE app_templates (
      id                CHAR(36)     NOT NULL PRIMARY KEY,
      slug              VARCHAR(64)  NOT NULL UNIQUE,
      name              VARCHAR(128) NOT NULL,
      icon_url          VARCHAR(512) NULL,
      category          VARCHAR(32)  NOT NULL DEFAULT ''app'',
      description       TEXT         NULL,
      visibility        VARCHAR(16)  NOT NULL DEFAULT ''PUBLIC'',
      owner_user_id     CHAR(36)     NULL,
      owner_project_id  CHAR(36)     NULL,
      spec              JSON         NOT NULL,
      requirements      JSON         NOT NULL,
      inputs            JSON         NOT NULL,
      is_active         TINYINT(1)   NOT NULL DEFAULT 1,
      version           INT          NOT NULL DEFAULT 1,
      created_at        DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
      updated_at        DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP
                                       ON UPDATE CURRENT_TIMESTAMP,
      INDEX idx_at_visibility (visibility),
      INDEX idx_at_owner_user (owner_user_id),
      INDEX idx_at_owner_project (owner_project_id),
      INDEX idx_at_category (category)
   ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

-- app_template_bindings: tracks which managed service instances a deployed
-- app is wired to. Populated in Phase 2; created here so migrations stay
-- monotonic.
SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.TABLES
   WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'app_template_bindings') = 0,
  'CREATE TABLE app_template_bindings (
      id               CHAR(36)    NOT NULL PRIMARY KEY,
      app_id           CHAR(36)    NOT NULL,
      requirement_key  VARCHAR(64) NOT NULL,
      binding_kind     VARCHAR(32) NOT NULL,
      binding_ref_id   CHAR(36)    NOT NULL,
      provisioned      TINYINT(1)  NOT NULL DEFAULT 0,
      created_at       DATETIME    NOT NULL DEFAULT CURRENT_TIMESTAMP,
      INDEX idx_atb_app (app_id),
      CONSTRAINT fk_atb_app FOREIGN KEY (app_id)
        REFERENCES apps(id) ON DELETE CASCADE
   ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;
