-- QuickStack initial schema
-- MySQL 5.7+ compatible, fully idempotent (safe to re-run)

CREATE TABLE IF NOT EXISTS platform_config (
  `key`        VARCHAR(100)  NOT NULL,
  `value`      TEXT          NOT NULL,
  description  VARCHAR(255)  NULL,
  updated_at   DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP
                             ON UPDATE CURRENT_TIMESTAMP,
  PRIMARY KEY (`key`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

INSERT IGNORE INTO platform_config (`key`, `value`, description) VALUES
  ('default_timezone',       'Asia/Shanghai', '默认时区'),
  ('platform_display_name',  'QuickStack',    '平台显示名称'),
  ('shared_storage_path',    '/storage',      '共享存储根路径'),
  ('nodeport_range_start',   '30000',         'NodePort 分配起始端口'),
  ('nodeport_range_end',     '32767',         'NodePort 分配结束端口'),
  ('nodeport_reserved',      '30100',         '保留 NodePort（逗号分隔），不会被自动分配');

-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS users (
  id                  CHAR(36)      NOT NULL,
  username            VARCHAR(64)   NOT NULL,
  email               VARCHAR(255)  NOT NULL,
  display_name        VARCHAR(128)  NULL,
  ldap_dn             VARCHAR(512)  NULL,
  ldap_uid            INT UNSIGNED  NULL         COMMENT 'POSIX UID',
  ldap_gid            INT UNSIGNED  NULL         COMMENT 'POSIX GID',
  is_global_admin     TINYINT(1)    NOT NULL DEFAULT 0,
  is_active           TINYINT(1)    NOT NULL DEFAULT 1,
  quota_cpu_mcores    INT UNSIGNED  NOT NULL DEFAULT 0,
  quota_mem_mb        INT UNSIGNED  NOT NULL DEFAULT 0,
  quota_storage_gb    INT UNSIGNED  NOT NULL DEFAULT 0,
  quota_apps          INT UNSIGNED  NOT NULL DEFAULT 0,
  quota_db_instances  INT UNSIGNED  NOT NULL DEFAULT 0,
  created_at          DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at          DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP
                                    ON UPDATE CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uq_users_username (username),
  UNIQUE KEY uq_users_email    (email)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS user_sessions (
  id          CHAR(36)     NOT NULL,
  user_id     CHAR(36)     NOT NULL,
  token_hash  VARCHAR(256) NOT NULL  COMMENT 'SHA-256(JWT)',
  ip_addr     VARCHAR(64)  NULL,
  user_agent  VARCHAR(512) NULL,
  expires_at  DATETIME     NOT NULL,
  created_at  DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY  uq_sessions_token  (token_hash),
  KEY         idx_sessions_user  (user_id),
  KEY         idx_sessions_expiry (expires_at),
  CONSTRAINT fk_sessions_user FOREIGN KEY (user_id)
    REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS totp_credentials (
  user_id    CHAR(36)     NOT NULL,
  secret     VARCHAR(512) NOT NULL  COMMENT 'AES-256-GCM encrypted TOTP secret',
  enabled    TINYINT(1)   NOT NULL DEFAULT 0,
  created_at DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (user_id),
  CONSTRAINT fk_totp_user FOREIGN KEY (user_id)
    REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS projects (
  id                  CHAR(36)     NOT NULL,
  name                VARCHAR(128) NOT NULL  COMMENT 'K8s namespace name',
  display_name        VARCHAR(255) NOT NULL,
  owner_id            CHAR(36)     NOT NULL,
  quota_cpu_mcores    INT UNSIGNED NOT NULL DEFAULT 0,
  quota_mem_mb        INT UNSIGNED NOT NULL DEFAULT 0,
  quota_storage_gb    INT UNSIGNED NOT NULL DEFAULT 0,
  quota_apps          INT UNSIGNED NOT NULL DEFAULT 0,
  quota_db_instances  INT UNSIGNED NOT NULL DEFAULT 0,
  is_active           TINYINT(1)   NOT NULL DEFAULT 1,
  created_at          DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at          DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP
                                   ON UPDATE CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uq_projects_name  (name),
  KEY        idx_projects_owner (owner_id),
  CONSTRAINT fk_projects_owner FOREIGN KEY (owner_id)
    REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS project_members (
  project_id  CHAR(36)                              NOT NULL,
  user_id     CHAR(36)                              NOT NULL,
  role        ENUM('ADMIN','OPERATOR','OBSERVER')   NOT NULL,
  added_by    CHAR(36)                              NULL,
  added_at    DATETIME                              NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (project_id, user_id),
  KEY idx_project_members_user (user_id),
  CONSTRAINT fk_pm_project    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
  CONSTRAINT fk_pm_user       FOREIGN KEY (user_id)    REFERENCES users(id)    ON DELETE CASCADE,
  CONSTRAINT fk_pm_added_by   FOREIGN KEY (added_by)   REFERENCES users(id)    ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS proxy_managers (
  id            CHAR(36)     NOT NULL,
  name          VARCHAR(128) NOT NULL,
  host          VARCHAR(255) NOT NULL,
  api_base_url  VARCHAR(512) NOT NULL  COMMENT 'http://{host}:81/api',
  api_username  VARCHAR(128) NOT NULL DEFAULT 'admin',
  api_password  VARCHAR(512) NOT NULL  COMMENT 'AES-256-GCM encrypted',
  is_active     TINYINT(1)   NOT NULL DEFAULT 1,
  created_at    DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uq_pm_name (name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS apps (
  id                      CHAR(36)      NOT NULL,
  project_id              CHAR(36)      NOT NULL,
  name                    VARCHAR(128)  NOT NULL,
  display_name            VARCHAR(255)  NULL,
  owner_id                CHAR(36)      NOT NULL,

  source_type             ENUM('GIT','CONTAINER') NOT NULL,
  container_image         VARCHAR(512)  NULL,
  container_registry_user VARCHAR(256)  NULL,
  container_registry_pass VARCHAR(512)  NULL      COMMENT 'AES-256-GCM encrypted',
  git_url                 VARCHAR(512)  NULL,
  git_branch              VARCHAR(128)  NULL,
  git_ref                 VARCHAR(256)  NULL,
  git_token               VARCHAR(512)  NULL      COMMENT 'AES-256-GCM encrypted',
  dockerfile_path         VARCHAR(512)  NULL DEFAULT 'Dockerfile',
  build_args              JSON          NULL,

  replicas                TINYINT UNSIGNED NOT NULL DEFAULT 1,
  container_command       VARCHAR(512)  NULL,
  container_args          JSON          NULL,
  working_dir             VARCHAR(512)  NULL,

  cpu_reservation_mcores  INT UNSIGNED  NULL,
  cpu_limit_mcores        INT UNSIGNED  NULL,
  mem_reservation_mb      INT UNSIGNED  NULL,
  mem_limit_mb            INT UNSIGNED  NULL,

  run_as_user             INT UNSIGNED  NULL,
  run_as_group            INT UNSIGNED  NULL,
  fs_group                INT UNSIGNED  NULL,
  privileged              TINYINT(1)   NOT NULL DEFAULT 0,
  read_only_root_fs       TINYINT(1)   NOT NULL DEFAULT 0,

  gpu_enabled             TINYINT(1)   NOT NULL DEFAULT 0,
  gpu_count               TINYINT UNSIGNED NOT NULL DEFAULT 0,
  gpu_model               VARCHAR(128)  NULL,

  mount_ldap_files        TINYINT(1)   NOT NULL DEFAULT 1,
  mount_etc_hosts         TINYINT(1)   NOT NULL DEFAULT 1,
  mount_user_home         TINYINT(1)   NOT NULL DEFAULT 1,
  mount_app_data          TINYINT(1)   NOT NULL DEFAULT 1,
  mount_app_logs          TINYINT(1)   NOT NULL DEFAULT 1,
  timezone                VARCHAR(64)  NOT NULL DEFAULT 'Asia/Shanghai',

  anti_affinity_enabled   TINYINT(1)   NOT NULL DEFAULT 1,

  health_check_type       ENUM('HTTP','TCP','NONE') NOT NULL DEFAULT 'NONE',
  health_check_path       VARCHAR(512)  NULL,
  health_check_port       SMALLINT UNSIGNED NULL,
  health_check_scheme     ENUM('HTTP','HTTPS') NULL DEFAULT 'HTTP',
  health_check_headers    JSON          NULL,
  health_check_period     SMALLINT UNSIGNED NOT NULL DEFAULT 10,
  health_check_timeout    SMALLINT UNSIGNED NOT NULL DEFAULT 5,
  health_check_failures   TINYINT UNSIGNED  NOT NULL DEFAULT 3,

  network_policy          ENUM('ALLOW_ALL','NAMESPACE_ONLY','DENY_ALL','INTERNET_ONLY')
                          NOT NULL DEFAULT 'ALLOW_ALL',

  status                  ENUM('STOPPED','DEPLOYING','RUNNING','FAILED','PAUSED')
                          NOT NULL DEFAULT 'STOPPED',
  paused_at               DATETIME      NULL,
  paused_by               CHAR(36)      NULL,
  pause_reason            VARCHAR(512)  NULL,

  webhook_id              CHAR(36)      NULL,

  created_at              DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at              DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP
                                        ON UPDATE CURRENT_TIMESTAMP,

  PRIMARY KEY (id),
  UNIQUE KEY  uq_app_name_project (project_id, name),
  UNIQUE KEY  uq_app_webhook      (webhook_id),
  KEY         idx_apps_project    (project_id),
  KEY         idx_apps_owner      (owner_id),
  CONSTRAINT fk_apps_project    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
  CONSTRAINT fk_apps_owner      FOREIGN KEY (owner_id)   REFERENCES users(id),
  CONSTRAINT fk_apps_paused_by  FOREIGN KEY (paused_by)  REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS app_env_vars (
  id        CHAR(36)      NOT NULL,
  app_id    CHAR(36)      NOT NULL,
  key_name  VARCHAR(256)  NOT NULL,
  value     TEXT          NULL,
  is_secret TINYINT(1)    NOT NULL DEFAULT 0,
  PRIMARY KEY (id),
  KEY idx_app_env_app (app_id),
  CONSTRAINT fk_env_app FOREIGN KEY (app_id) REFERENCES apps(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS app_ports (
  id              CHAR(36)          NOT NULL,
  app_id          CHAR(36)          NOT NULL,
  container_port  SMALLINT UNSIGNED NOT NULL,
  protocol        ENUM('TCP','UDP') NOT NULL DEFAULT 'TCP',
  nodeport        SMALLINT UNSIGNED NULL,
  PRIMARY KEY (id),
  UNIQUE KEY uq_app_ports_nodeport (nodeport),
  KEY idx_app_ports_app (app_id),
  CONSTRAINT fk_ports_app FOREIGN KEY (app_id) REFERENCES apps(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS app_domains (
  id                CHAR(36)     NOT NULL,
  app_id            CHAR(36)     NOT NULL,
  hostname          VARCHAR(512) NOT NULL,
  target_port       SMALLINT UNSIGNED NOT NULL,
  is_system_domain  TINYINT(1)   NOT NULL DEFAULT 0,
  ssl_enabled       TINYINT(1)   NOT NULL DEFAULT 1,
  use_lets_encrypt  TINYINT(1)   NOT NULL DEFAULT 1,
  cert_status       ENUM('NONE','PENDING','ISSUED','FAILED') NOT NULL DEFAULT 'NONE',
  cert_expiry       DATETIME     NULL,
  basic_auth_enabled TINYINT(1)  NOT NULL DEFAULT 0,
  basic_auth_user   VARCHAR(128) NULL,
  basic_auth_pass   VARCHAR(512) NULL  COMMENT 'AES-256-GCM encrypted',
  redirect_https    TINYINT(1)   NOT NULL DEFAULT 1,
  created_at        DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY  uq_domain_hostname (hostname),
  KEY         idx_domains_app    (app_id),
  CONSTRAINT fk_domains_app FOREIGN KEY (app_id) REFERENCES apps(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS app_file_mounts (
  id          CHAR(36)      NOT NULL,
  app_id      CHAR(36)      NOT NULL,
  mount_path  VARCHAR(512)  NOT NULL,
  filename    VARCHAR(255)  NOT NULL,
  content     LONGTEXT      NOT NULL,
  PRIMARY KEY (id),
  KEY idx_file_mounts_app (app_id),
  CONSTRAINT fk_file_mounts_app FOREIGN KEY (app_id) REFERENCES apps(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS app_extra_volumes (
  id          CHAR(36)     NOT NULL,
  app_id      CHAR(36)     NOT NULL,
  host_path   VARCHAR(512) NOT NULL,
  mount_path  VARCHAR(512) NOT NULL,
  read_only   TINYINT(1)   NOT NULL DEFAULT 0,
  PRIMARY KEY (id),
  KEY idx_extra_vols_app (app_id),
  CONSTRAINT fk_extra_vols_app FOREIGN KEY (app_id) REFERENCES apps(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS build_jobs (
  id               CHAR(36)     NOT NULL,
  app_id           CHAR(36)     NOT NULL,
  k8s_job_name     VARCHAR(256) NULL,
  git_commit_hash  VARCHAR(64)  NULL,
  image_tag        VARCHAR(256) NULL,
  status           ENUM('PENDING','RUNNING','SUCCEEDED','FAILED','CANCELLED')
                   NOT NULL DEFAULT 'PENDING',
  triggered_by     CHAR(36)     NULL,
  trigger_type     ENUM('MANUAL','WEBHOOK','API') NOT NULL DEFAULT 'MANUAL',
  started_at       DATETIME     NULL,
  finished_at      DATETIME     NULL,
  created_at       DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  KEY idx_build_jobs_app    (app_id),
  KEY idx_build_jobs_status (status),
  CONSTRAINT fk_build_jobs_app  FOREIGN KEY (app_id)       REFERENCES apps(id) ON DELETE CASCADE,
  CONSTRAINT fk_build_jobs_user FOREIGN KEY (triggered_by) REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS deployment_events (
  id           CHAR(36)     NOT NULL,
  app_id       CHAR(36)     NOT NULL,
  event_type   ENUM('DEPLOY','ROLLBACK','SCALE','PAUSE','RESUME','CONFIG_CHANGE') NOT NULL,
  status       ENUM('PENDING','RUNNING','SUCCEEDED','FAILED') NOT NULL DEFAULT 'PENDING',
  triggered_by CHAR(36)     NULL,
  message      TEXT         NULL,
  created_at   DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  KEY idx_deploy_events_app  (app_id),
  KEY idx_deploy_events_time (created_at),
  CONSTRAINT fk_deploy_events_app  FOREIGN KEY (app_id)       REFERENCES apps(id) ON DELETE CASCADE,
  CONSTRAINT fk_deploy_events_user FOREIGN KEY (triggered_by) REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS database_clusters (
  id              CHAR(36)     NOT NULL,
  name            VARCHAR(128) NOT NULL,
  cluster_type    ENUM('MYSQL_GALERA','POSTGRESQL') NOT NULL,
  host            VARCHAR(512) NOT NULL,
  port            SMALLINT UNSIGNED NOT NULL,
  admin_user      VARCHAR(128) NOT NULL,
  admin_password  VARCHAR(512) NOT NULL  COMMENT 'AES-256-GCM encrypted',
  max_databases   INT UNSIGNED NOT NULL DEFAULT 0,
  is_active       TINYINT(1)   NOT NULL DEFAULT 1,
  description     VARCHAR(512) NULL,
  created_at      DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at      DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP
                               ON UPDATE CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uq_cluster_name (name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS database_instances (
  id               CHAR(36)     NOT NULL,
  cluster_id       CHAR(36)     NOT NULL,
  project_id       CHAR(36)     NOT NULL,
  created_by       CHAR(36)     NOT NULL,
  db_name          VARCHAR(128) NOT NULL,
  db_user          VARCHAR(128) NOT NULL,
  db_password      VARCHAR(512) NOT NULL  COMMENT 'AES-256-GCM encrypted',
  k8s_secret_name  VARCHAR(256) NULL,
  status           ENUM('ACTIVE','SUSPENDED','DROPPED') NOT NULL DEFAULT 'ACTIVE',
  created_at       DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY  uq_db_name_cluster  (cluster_id, db_name),
  KEY         idx_db_inst_project (project_id),
  KEY         idx_db_inst_cluster (cluster_id),
  CONSTRAINT fk_db_inst_cluster  FOREIGN KEY (cluster_id)  REFERENCES database_clusters(id),
  CONSTRAINT fk_db_inst_project  FOREIGN KEY (project_id)  REFERENCES projects(id) ON DELETE CASCADE,
  CONSTRAINT fk_db_inst_creator  FOREIGN KEY (created_by)  REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS cluster_nodes (
  id                    CHAR(36)     NOT NULL,
  hostname              VARCHAR(255) NOT NULL,
  ip_address            VARCHAR(64)  NOT NULL,
  node_role             ENUM('MASTER','WORKER') NOT NULL DEFAULT 'WORKER',
  has_gpu               TINYINT(1)   NOT NULL DEFAULT 0,
  gpu_model             VARCHAR(128) NULL,
  gpu_count             TINYINT UNSIGNED NOT NULL DEFAULT 0,
  k8s_labels            JSON         NULL,
  node_status           ENUM('READY','NOT_READY','UNKNOWN') NOT NULL DEFAULT 'UNKNOWN',
  cpu_capacity_mcores   INT UNSIGNED NULL,
  mem_capacity_mb       INT UNSIGNED NULL,
  storage_available     TINYINT(1)   NOT NULL DEFAULT 0,
  ldap_auth_active      TINYINT(1)   NOT NULL DEFAULT 0,
  last_seen_at          DATETIME     NULL,
  created_at            DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uq_node_hostname (hostname)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS s3_targets (
  id             CHAR(36)     NOT NULL,
  name           VARCHAR(128) NOT NULL,
  endpoint       VARCHAR(512) NOT NULL,
  region         VARCHAR(128) NULL,
  access_key_id  VARCHAR(256) NOT NULL,
  secret_key     VARCHAR(512) NOT NULL  COMMENT 'AES-256-GCM encrypted',
  bucket_name    VARCHAR(256) NOT NULL,
  is_active      TINYINT(1)   NOT NULL DEFAULT 1,
  created_at     DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uq_s3_name (name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ------------------------------------------------------------

CREATE TABLE IF NOT EXISTS backup_schedules (
  id              CHAR(36)     NOT NULL,
  app_id          CHAR(36)     NOT NULL,
  s3_target_id    CHAR(36)     NOT NULL,
  cron_expr       VARCHAR(128) NOT NULL,
  retention_days  SMALLINT UNSIGNED NOT NULL DEFAULT 7,
  backup_type     ENUM('FILES','DB_DUMP','BOTH') NOT NULL DEFAULT 'FILES',
  db_instance_id  CHAR(36)     NULL,
  is_active       TINYINT(1)   NOT NULL DEFAULT 1,
  last_run_at     DATETIME     NULL,
  last_run_status ENUM('SUCCESS','FAILED','RUNNING') NULL,
  created_at      DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  KEY idx_backup_app (app_id),
  CONSTRAINT fk_backup_app FOREIGN KEY (app_id)          REFERENCES apps(id) ON DELETE CASCADE,
  CONSTRAINT fk_backup_s3  FOREIGN KEY (s3_target_id)    REFERENCES s3_targets(id),
  CONSTRAINT fk_backup_db  FOREIGN KEY (db_instance_id)  REFERENCES database_instances(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
