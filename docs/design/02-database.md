# QuickStack 数据库设计文档

版本：1.0  
日期：2026-05-22  
状态：草稿

数据库：MySQL 8.0+  
字符集：`utf8mb4`  
排序规则：`utf8mb4_unicode_ci`

---

## 1. ER 图（核心关系）

```
users ──────────────────────────── project_members ─── projects
  │                                                        │
  │ (owner_id)                                             │ (project_id)
  │                                                        │
  ├── totp_credentials                                     ├── apps
  │                                                        │     │
  └── user_sessions                                        │     ├── app_env_vars
                                                           │     ├── app_ports
                                                           │     ├── app_domains
                                                           │     ├── app_file_mounts
                                                           │     ├── app_extra_volumes
                                                           │     ├── build_jobs
                                                           │     └── deployment_events
                                                           │
                                                           ├── database_instances ── database_clusters
                                                           │
                                                           └── backup_schedules ── s3_targets

cluster_nodes（独立，与 K8s 节点同步）
platform_config（键值对全局配置）
```

---

## 2. 完整建表 DDL

### 2.1 全局配置表

```sql
CREATE TABLE platform_config (
  `key`        VARCHAR(100)  NOT NULL,
  `value`      TEXT          NOT NULL,
  description  VARCHAR(255)  NULL,
  updated_at   DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP
                             ON UPDATE CURRENT_TIMESTAMP,
  PRIMARY KEY (`key`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

**预置键值：**

| key | 默认值 | 说明 |
|-----|--------|------|
| `platform_domain` | — | 平台管理域名 |
| `app_subdomain_base` | — | 应用二级域名后缀 |
| `default_timezone` | `Asia/Shanghai` | 默认时区 |
| `platform_display_name` | `QuickStack` | 平台显示名称 |
| `shared_storage_path` | `/storage` | 共享存储根路径 |
| `ldap_url` | — | LLDAP 地址 |
| `ldap_base_dn` | — | LDAP Base DN |
| `ldap_bind_dn` | — | LDAP Bind DN |
| `ldap_bind_password` | — | 加密存储 |
| `ldap_user_ou` | `ou=people` | 用户 OU |
| `ldap_group_ou` | `ou=groups` | 组 OU |
| `ldap_sync_interval_sec` | `600` | 同步间隔（秒）|
| `ssh_public_key` | — | 平台 SSH 公钥 |
| `ssh_private_key` | — | 加密存储 |
| `jwt_secret` | — | JWT 签名密钥，加密存储 |
| `registry_host` | — | 内部镜像仓库地址 |

---

### 2.2 用户表

```sql
CREATE TABLE users (
  id                  CHAR(36)      NOT NULL,
  username            VARCHAR(64)   NOT NULL,
  email               VARCHAR(255)  NOT NULL,
  display_name        VARCHAR(128)  NULL,
  ldap_dn             VARCHAR(512)  NULL,
  ldap_uid            INT UNSIGNED  NULL         COMMENT 'POSIX UID，用于容器 securityContext',
  ldap_gid            INT UNSIGNED  NULL         COMMENT 'POSIX 主 GID',
  is_global_admin     TINYINT(1)    NOT NULL DEFAULT 0,
  is_active           TINYINT(1)    NOT NULL DEFAULT 1,
  -- 用户级配额（0 = 不限制）
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
```

---

### 2.3 用户会话表

```sql
CREATE TABLE user_sessions (
  id          CHAR(36)     NOT NULL,
  user_id     CHAR(36)     NOT NULL,
  token_hash  VARCHAR(256) NOT NULL  COMMENT 'SHA-256(JWT token)',
  ip_addr     VARCHAR(64)  NULL,
  user_agent  VARCHAR(512) NULL,
  expires_at  DATETIME     NOT NULL,
  created_at  DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY  uq_sessions_token (token_hash),
  KEY         idx_sessions_user   (user_id),
  KEY         idx_sessions_expiry (expires_at),
  CONSTRAINT fk_sessions_user FOREIGN KEY (user_id)
    REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

---

### 2.4 TOTP 凭据表

```sql
CREATE TABLE totp_credentials (
  user_id    CHAR(36)     NOT NULL,
  secret     VARCHAR(512) NOT NULL  COMMENT 'AES-256-GCM 加密的 TOTP secret',
  enabled    TINYINT(1)   NOT NULL DEFAULT 0,
  created_at DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (user_id),
  CONSTRAINT fk_totp_user FOREIGN KEY (user_id)
    REFERENCES users(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

---

### 2.5 项目表

```sql
CREATE TABLE projects (
  id                  CHAR(36)     NOT NULL,
  name                VARCHAR(128) NOT NULL  COMMENT '同 K8s namespace，仅小写字母数字和连字符',
  display_name        VARCHAR(255) NOT NULL,
  owner_id            CHAR(36)     NOT NULL,
  -- 项目级配额（0 = 不限制）
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
  UNIQUE KEY uq_projects_name (name),
  KEY        idx_projects_owner (owner_id),
  CONSTRAINT fk_projects_owner FOREIGN KEY (owner_id)
    REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

---

### 2.6 项目成员表

```sql
CREATE TABLE project_members (
  project_id  CHAR(36)                         NOT NULL,
  user_id     CHAR(36)                         NOT NULL,
  role        ENUM('ADMIN','OPERATOR','OBSERVER') NOT NULL,
  added_by    CHAR(36)                         NULL,
  added_at    DATETIME                         NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (project_id, user_id),
  KEY idx_project_members_user (user_id),
  CONSTRAINT fk_pm_project FOREIGN KEY (project_id)
    REFERENCES projects(id) ON DELETE CASCADE,
  CONSTRAINT fk_pm_user FOREIGN KEY (user_id)
    REFERENCES users(id) ON DELETE CASCADE,
  CONSTRAINT fk_pm_added_by FOREIGN KEY (added_by)
    REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

---

### 2.7 反向代理管理器配置表

平台对接唯一的 pingora-proxy-manager 实例（支持配置多个用于高可用，取第一个 `is_active=1` 的）。

```sql
CREATE TABLE proxy_managers (
  id            CHAR(36)     NOT NULL,
  name          VARCHAR(128) NOT NULL,
  host          VARCHAR(255) NOT NULL                 COMMENT '部署 pingora 的主机 IP/域名',
  api_base_url  VARCHAR(512) NOT NULL                 COMMENT 'http://{host}:81/api',
  api_username  VARCHAR(128) NOT NULL DEFAULT 'admin' COMMENT 'pingora 管理账号',
  api_password  VARCHAR(512) NOT NULL                 COMMENT 'AES-256-GCM 加密',
  -- JWT token 由平台在启动时通过 POST /login 获取，缓存在内存中，过期后自动续签
  is_active     TINYINT(1)   NOT NULL DEFAULT 1,
  created_at    DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uq_pm_name (name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

---

### 2.8 应用表

```sql
CREATE TABLE apps (
  id                      CHAR(36)      NOT NULL,
  project_id              CHAR(36)      NOT NULL,
  name                    VARCHAR(128)  NOT NULL  COMMENT 'DNS 安全名称',
  display_name            VARCHAR(255)  NULL,
  owner_id                CHAR(36)      NOT NULL,

  -- 来源
  source_type             ENUM('GIT','CONTAINER') NOT NULL,
  container_image         VARCHAR(512)  NULL,
  container_registry_user VARCHAR(256)  NULL,
  container_registry_pass VARCHAR(512)  NULL      COMMENT 'AES-256-GCM 加密',
  git_url                 VARCHAR(512)  NULL,
  git_branch              VARCHAR(128)  NULL,
  git_ref                 VARCHAR(256)  NULL,
  git_token               VARCHAR(512)  NULL      COMMENT 'AES-256-GCM 加密',
  dockerfile_path         VARCHAR(512)  NULL      DEFAULT 'Dockerfile',
  build_args              JSON          NULL      COMMENT '[{key, value}]',

  -- 运行配置
  replicas                TINYINT UNSIGNED NOT NULL DEFAULT 1,
  container_command       VARCHAR(512)  NULL,
  container_args          JSON          NULL      COMMENT 'string[]',
  working_dir             VARCHAR(512)  NULL,

  -- 资源限制
  cpu_reservation_mcores  INT UNSIGNED  NULL,
  cpu_limit_mcores        INT UNSIGNED  NULL,
  mem_reservation_mb      INT UNSIGNED  NULL,
  mem_limit_mb            INT UNSIGNED  NULL,

  -- SecurityContext
  run_as_user             INT UNSIGNED  NULL      COMMENT 'NULL 则使用 ldap_uid',
  run_as_group            INT UNSIGNED  NULL,
  fs_group                INT UNSIGNED  NULL,
  privileged              TINYINT(1)   NOT NULL DEFAULT 0,
  read_only_root_fs       TINYINT(1)   NOT NULL DEFAULT 0,

  -- GPU
  gpu_enabled             TINYINT(1)   NOT NULL DEFAULT 0,
  gpu_count               TINYINT UNSIGNED NOT NULL DEFAULT 0,
  gpu_model               VARCHAR(128)  NULL,

  -- 标准挂载开关
  mount_ldap_files        TINYINT(1)   NOT NULL DEFAULT 1,
  mount_etc_hosts         TINYINT(1)   NOT NULL DEFAULT 1,
  mount_user_home         TINYINT(1)   NOT NULL DEFAULT 1,
  mount_app_data          TINYINT(1)   NOT NULL DEFAULT 1,
  mount_app_logs          TINYINT(1)   NOT NULL DEFAULT 1,
  timezone                VARCHAR(64)  NOT NULL DEFAULT 'Asia/Shanghai',

  -- 调度
  anti_affinity_enabled   TINYINT(1)   NOT NULL DEFAULT 1,

  -- 健康检查
  health_check_type       ENUM('HTTP','TCP','NONE') NOT NULL DEFAULT 'NONE',
  health_check_path       VARCHAR(512)  NULL,
  health_check_port       SMALLINT UNSIGNED NULL,
  health_check_scheme     ENUM('HTTP','HTTPS') NULL DEFAULT 'HTTP',
  health_check_headers    JSON          NULL,
  health_check_period     SMALLINT UNSIGNED NOT NULL DEFAULT 10,
  health_check_timeout    SMALLINT UNSIGNED NOT NULL DEFAULT 5,
  health_check_failures   TINYINT UNSIGNED  NOT NULL DEFAULT 3,

  -- 网络策略
  network_policy          ENUM('ALLOW_ALL','NAMESPACE_ONLY','DENY_ALL','INTERNET_ONLY')
                          NOT NULL DEFAULT 'ALLOW_ALL',

  -- 状态
  status                  ENUM('STOPPED','DEPLOYING','RUNNING','FAILED','PAUSED')
                          NOT NULL DEFAULT 'STOPPED',
  paused_at               DATETIME      NULL,
  paused_by               CHAR(36)      NULL,
  pause_reason            VARCHAR(512)  NULL,

  -- Webhook
  webhook_id              CHAR(36)      NULL,

  created_at              DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at              DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP
                                        ON UPDATE CURRENT_TIMESTAMP,

  PRIMARY KEY (id),
  UNIQUE KEY  uq_app_name_project (project_id, name),
  UNIQUE KEY  uq_app_webhook      (webhook_id),
  KEY         idx_apps_project    (project_id),
  KEY         idx_apps_owner      (owner_id),
  CONSTRAINT fk_apps_project FOREIGN KEY (project_id)
    REFERENCES projects(id) ON DELETE CASCADE,
  CONSTRAINT fk_apps_owner FOREIGN KEY (owner_id)
    REFERENCES users(id),
  CONSTRAINT fk_apps_paused_by FOREIGN KEY (paused_by)
    REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

---

### 2.9 应用环境变量表

```sql
CREATE TABLE app_env_vars (
  id        CHAR(36)      NOT NULL,
  app_id    CHAR(36)      NOT NULL,
  key_name  VARCHAR(256)  NOT NULL,
  value     TEXT          NULL,
  is_secret TINYINT(1)    NOT NULL DEFAULT 0  COMMENT '1 则 value 为 AES-256-GCM 加密',
  PRIMARY KEY (id),
  KEY idx_app_env_app (app_id),
  CONSTRAINT fk_env_app FOREIGN KEY (app_id)
    REFERENCES apps(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

---

### 2.10 应用端口表

```sql
CREATE TABLE app_ports (
  id              CHAR(36)          NOT NULL,
  app_id          CHAR(36)          NOT NULL,
  container_port  SMALLINT UNSIGNED NOT NULL,
  protocol        ENUM('TCP','UDP') NOT NULL DEFAULT 'TCP',
  -- NodePort 由平台在首次部署时从可配置范围分配（platform_config: nodeport_range_start/end），创建 K8s Service 后写入
  nodeport        SMALLINT UNSIGNED NULL     COMMENT 'K8s NodePort，Pingora 上游使用此端口',
  PRIMARY KEY (id),
  UNIQUE KEY uq_app_ports_nodeport (nodeport),   -- 全局唯一，防止端口冲突
  KEY idx_app_ports_app (app_id),
  CONSTRAINT fk_ports_app FOREIGN KEY (app_id)
    REFERENCES apps(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

> **NodePort 分配策略：** 平台在部署应用时，从 `app_ports.nodeport` 的已用集合中找出 30000–32767 范围内未使用的最小端口，创建 K8s `NodePort` Service 后写回此字段。删除应用时端口随 Service 一起释放，字段置 NULL，可被后续应用复用。

---

### 2.11 应用域名表

```sql
CREATE TABLE app_domains (
  id                CHAR(36)     NOT NULL,
  app_id            CHAR(36)     NOT NULL,
  hostname          VARCHAR(512) NOT NULL,
  target_port       SMALLINT UNSIGNED NOT NULL,
  is_system_domain  TINYINT(1)   NOT NULL DEFAULT 0  COMMENT '平台自动分配不可删除',
  ssl_enabled       TINYINT(1)   NOT NULL DEFAULT 1,
  use_lets_encrypt  TINYINT(1)   NOT NULL DEFAULT 1,
  cert_status        ENUM('NONE','PENDING','ISSUED','FAILED') NOT NULL DEFAULT 'NONE',
  cert_expiry        DATETIME     NULL,
  -- pingora 中对应 proxy host 由平台在创建/删除域名时通过 API 同步维护，无需外键
  basic_auth_enabled TINYINT(1)   NOT NULL DEFAULT 0,
  basic_auth_user    VARCHAR(128) NULL,
  basic_auth_pass    VARCHAR(512) NULL  COMMENT 'AES-256-GCM 加密',
  redirect_https     TINYINT(1)   NOT NULL DEFAULT 1,
  created_at         DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY  uq_domain_hostname (hostname),
  KEY         idx_domains_app    (app_id),
  CONSTRAINT fk_domains_app FOREIGN KEY (app_id)
    REFERENCES apps(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

---

### 2.12 应用文件挂载表

```sql
CREATE TABLE app_file_mounts (
  id          CHAR(36)      NOT NULL,
  app_id      CHAR(36)      NOT NULL,
  mount_path  VARCHAR(512)  NOT NULL  COMMENT '容器内目录路径',
  filename    VARCHAR(255)  NOT NULL,
  content     LONGTEXT      NOT NULL,
  PRIMARY KEY (id),
  KEY idx_file_mounts_app (app_id),
  CONSTRAINT fk_file_mounts_app FOREIGN KEY (app_id)
    REFERENCES apps(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

---

### 2.13 应用额外卷表

```sql
CREATE TABLE app_extra_volumes (
  id          CHAR(36)     NOT NULL,
  app_id      CHAR(36)     NOT NULL,
  host_path   VARCHAR(512) NOT NULL  COMMENT '宿主机路径，必须以 /storage/{username}/ 开头',
  mount_path  VARCHAR(512) NOT NULL  COMMENT '容器内路径',
  read_only   TINYINT(1)   NOT NULL DEFAULT 0,
  PRIMARY KEY (id),
  KEY idx_extra_vols_app (app_id),
  CONSTRAINT fk_extra_vols_app FOREIGN KEY (app_id)
    REFERENCES apps(id) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

---

### 2.14 构建任务表

```sql
CREATE TABLE build_jobs (
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
  CONSTRAINT fk_build_jobs_app FOREIGN KEY (app_id)
    REFERENCES apps(id) ON DELETE CASCADE,
  CONSTRAINT fk_build_jobs_user FOREIGN KEY (triggered_by)
    REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

---

### 2.15 部署事件表

```sql
CREATE TABLE deployment_events (
  id           CHAR(36)     NOT NULL,
  app_id       CHAR(36)     NOT NULL,
  event_type   ENUM('DEPLOY','ROLLBACK','SCALE','PAUSE','RESUME','CONFIG_CHANGE')
               NOT NULL,
  status       ENUM('PENDING','RUNNING','SUCCEEDED','FAILED') NOT NULL DEFAULT 'PENDING',
  triggered_by CHAR(36)     NULL,
  message      TEXT         NULL,
  created_at   DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  KEY idx_deploy_events_app    (app_id),
  KEY idx_deploy_events_time   (created_at),
  CONSTRAINT fk_deploy_events_app FOREIGN KEY (app_id)
    REFERENCES apps(id) ON DELETE CASCADE,
  CONSTRAINT fk_deploy_events_user FOREIGN KEY (triggered_by)
    REFERENCES users(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

---

### 2.16 数据库集群表

```sql
CREATE TABLE database_clusters (
  id              CHAR(36)     NOT NULL,
  name            VARCHAR(128) NOT NULL,
  cluster_type    ENUM('MYSQL_GALERA','POSTGRESQL') NOT NULL,
  host            VARCHAR(512) NOT NULL,
  port            SMALLINT UNSIGNED NOT NULL,
  admin_user      VARCHAR(128) NOT NULL,
  admin_password  VARCHAR(512) NOT NULL  COMMENT 'AES-256-GCM 加密',
  max_databases   INT UNSIGNED NOT NULL DEFAULT 0  COMMENT '0 = 不限',
  is_active       TINYINT(1)   NOT NULL DEFAULT 1,
  description     VARCHAR(512) NULL,
  created_at      DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at      DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP
                               ON UPDATE CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uq_cluster_name (name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

---

### 2.17 数据库实例表

```sql
CREATE TABLE database_instances (
  id               CHAR(36)     NOT NULL,
  cluster_id       CHAR(36)     NOT NULL,
  project_id       CHAR(36)     NOT NULL,
  created_by       CHAR(36)     NOT NULL,
  db_name          VARCHAR(128) NOT NULL  COMMENT '实际数据库名（含前缀）',
  db_user          VARCHAR(128) NOT NULL,
  db_password      VARCHAR(512) NOT NULL  COMMENT 'AES-256-GCM 加密',
  k8s_secret_name  VARCHAR(256) NULL      COMMENT '同项目 namespace 的 Secret 名称',
  status           ENUM('ACTIVE','SUSPENDED','DROPPED') NOT NULL DEFAULT 'ACTIVE',
  created_at       DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY  uq_db_name_cluster  (cluster_id, db_name),
  KEY         idx_db_inst_project (project_id),
  KEY         idx_db_inst_cluster (cluster_id),
  CONSTRAINT fk_db_inst_cluster FOREIGN KEY (cluster_id)
    REFERENCES database_clusters(id),
  CONSTRAINT fk_db_inst_project FOREIGN KEY (project_id)
    REFERENCES projects(id) ON DELETE CASCADE,
  CONSTRAINT fk_db_inst_creator FOREIGN KEY (created_by)
    REFERENCES users(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

---

### 2.18 节点表

```sql
CREATE TABLE cluster_nodes (
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
  storage_available     TINYINT(1)   NOT NULL DEFAULT 0  COMMENT '共享存储挂载状态',
  ldap_auth_active      TINYINT(1)   NOT NULL DEFAULT 0  COMMENT 'nslcd 运行状态',
  last_seen_at          DATETIME     NULL,
  created_at            DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uq_node_hostname (hostname)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

---

### 2.19 S3 备份目标表

```sql
CREATE TABLE s3_targets (
  id             CHAR(36)     NOT NULL,
  name           VARCHAR(128) NOT NULL,
  endpoint       VARCHAR(512) NOT NULL,
  region         VARCHAR(128) NULL,
  access_key_id  VARCHAR(256) NOT NULL,
  secret_key     VARCHAR(512) NOT NULL  COMMENT 'AES-256-GCM 加密',
  bucket_name    VARCHAR(256) NOT NULL,
  is_active      TINYINT(1)   NOT NULL DEFAULT 1,
  created_at     DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uq_s3_name (name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

---

### 2.20 备份计划表

```sql
CREATE TABLE backup_schedules (
  id             CHAR(36)     NOT NULL,
  app_id         CHAR(36)     NOT NULL,
  s3_target_id   CHAR(36)     NOT NULL,
  cron_expr      VARCHAR(128) NOT NULL,
  retention_days SMALLINT UNSIGNED NOT NULL DEFAULT 7,
  backup_type    ENUM('FILES','DB_DUMP','BOTH') NOT NULL DEFAULT 'FILES',
  db_instance_id CHAR(36)     NULL,
  is_active      TINYINT(1)   NOT NULL DEFAULT 1,
  last_run_at    DATETIME     NULL,
  last_run_status ENUM('SUCCESS','FAILED','RUNNING') NULL,
  created_at     DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  KEY idx_backup_app (app_id),
  CONSTRAINT fk_backup_app FOREIGN KEY (app_id)
    REFERENCES apps(id) ON DELETE CASCADE,
  CONSTRAINT fk_backup_s3 FOREIGN KEY (s3_target_id)
    REFERENCES s3_targets(id),
  CONSTRAINT fk_backup_db FOREIGN KEY (db_instance_id)
    REFERENCES database_instances(id) ON DELETE SET NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
```

---

## 3. 索引设计说明

| 表 | 索引 | 目的 |
|----|------|------|
| `users` | `uq_username`, `uq_email` | 登录查询唯一性 |
| `user_sessions` | `uq_token_hash`, `idx_expiry` | 会话验证 + 过期清理 |
| `project_members` | 复合主键 `(project_id, user_id)`, `idx_user` | 权限检查双向查询 |
| `apps` | `uq_(project_id, name)`, `idx_project`, `idx_owner` | 列表查询 + 唯一性 |
| `app_domains` | `uq_hostname` | 域名全局唯一 |
| `database_instances` | `uq_(cluster_id, db_name)` | 防重复建库 |
| `deployment_events` | `idx_app`, `idx_time` | 应用历史 + 时间排序 |
| `build_jobs` | `idx_app`, `idx_status` | 构建历史 + 状态过滤 |

---

## 4. 加密字段说明

所有敏感字段在存入数据库前使用 AES-256-GCM 加密，加密密钥来源于启动时从环境变量读取的 `QS_ENCRYPTION_KEY`（32 字节，Base64 编码）。

| 表 | 字段 | 说明 |
|----|------|------|
| `platform_config` | `ldap_bind_password`, `ssh_private_key`, `jwt_secret` | 平台核心凭据 |
| `users` | — | 密码存于 LLDAP，平台不存储 |
| `totp_credentials` | `secret` | TOTP 种子 |
| `load_balancers` | `api_token` | Pingora API 令牌 |
| `apps` | `container_registry_pass`, `git_token` | 拉取凭据 |
| `app_env_vars` | `value`（is_secret=1 时）| 应用 Secret 环境变量 |
| `app_domains` | `basic_auth_pass` | HTTP Basic Auth 密码 |
| `database_clusters` | `admin_password` | 集群管理凭据 |
| `database_instances` | `db_password` | 租户数据库密码 |
| `s3_targets` | `secret_key` | S3 Secret Key |

---

## 5. 数据字典

### users

| 字段 | 类型 | 可空 | 说明 |
|------|------|------|------|
| id | CHAR(36) | NO | UUID v4 |
| username | VARCHAR(64) | NO | LDAP uid，全局唯一 |
| email | VARCHAR(255) | NO | 来自 LDAP mail |
| display_name | VARCHAR(128) | YES | 来自 LDAP cn |
| ldap_dn | VARCHAR(512) | YES | LDAP 完整 DN |
| ldap_uid | INT UNSIGNED | YES | POSIX UID，用于 Pod securityContext |
| ldap_gid | INT UNSIGNED | YES | POSIX 主 GID |
| is_global_admin | TINYINT(1) | NO | 对应 LLDAP lldap_admin 组 |
| is_active | TINYINT(1) | NO | 禁用后无法登录 |
| quota_* | INT UNSIGNED | NO | 0 = 不限 |

### apps.status 状态说明

| 值 | 说明 | K8s 状态 |
|----|------|---------|
| STOPPED | 未部署或已停止 | Deployment 不存在或 replicas=0 |
| DEPLOYING | 部署中 | Deployment 存在，Pod 未就绪 |
| RUNNING | 运行中 | 所有 Pod Ready |
| FAILED | 部署失败 | Pod CrashLoopBackOff 或 Job Failed |
| PAUSED | 已暂停（审计状态）| replicas=0，Ingress 返回 503 |

### database_instances.db_name 命名规则

```
格式：p_{project_name}_{user_defined_name}
示例：p_myproject_appdb

database_instances.db_user 命名规则：
格式：u_{username}_{random6位小写字母数字}
示例：u_alice_a3f9x2
```

---

## 6. 迁移策略

采用 `sqlx` 内置迁移机制，迁移文件位于 `backend/src/db/migrations/`，命名格式：

```
001_init_schema.sql
002_add_gpu_support.sql
003_...
```

启动时自动执行未应用的迁移，所有 DDL 变更通过迁移文件管理，禁止手动修改生产数据库结构。
