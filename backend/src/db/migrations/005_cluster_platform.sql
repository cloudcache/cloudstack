-- Migration 005: image registries, IPAM, DB cluster manager_url

-- Add web-based manager URL to database clusters (phpMyAdmin, pgAdmin, etc.)
SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='database_clusters' AND COLUMN_NAME='manager_url') = 0,
  'ALTER TABLE database_clusters ADD COLUMN `manager_url` VARCHAR(512) NULL AFTER `description`',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

-- ── Image registries ────────────────────────────────────────────────────────
-- Ordered pull sources: custom registry first, Docker Hub as fallback.
-- priority: lower number = higher priority. Set is_default=1 for the fallback.
CREATE TABLE IF NOT EXISTS image_registries (
  id          CHAR(36)      NOT NULL,
  name        VARCHAR(128)  NOT NULL,
  endpoint    VARCHAR(512)  NOT NULL,          -- e.g. registry.example.com or docker.io
  username    VARCHAR(256)  NULL,
  password    VARCHAR(1024) NULL               COMMENT 'AES-256-GCM encrypted',
  is_default  TINYINT(1)    NOT NULL DEFAULT 0, -- fallback registry (Docker Hub)
  priority    SMALLINT      NOT NULL DEFAULT 100,
  is_active   TINYINT(1)    NOT NULL DEFAULT 1,
  created_at  DATETIME      NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uq_registry_name (name)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- ── IPAM ────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS ip_pools (
  id          CHAR(36)     NOT NULL,
  name        VARCHAR(128) NOT NULL,
  cidr        VARCHAR(64)  NOT NULL,           -- e.g. 192.168.1.128/26
  pool_type   ENUM('LB','NODE','CUSTOM') NOT NULL DEFAULT 'LB',
  gateway     VARCHAR(64)  NULL,
  description VARCHAR(512) NULL,
  is_active   TINYINT(1)   NOT NULL DEFAULT 1,
  created_at  DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uq_pool_name (name),
  UNIQUE KEY uq_pool_cidr (cidr)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;

-- Active IP allocations. Row is deleted when the IP is released.
CREATE TABLE IF NOT EXISTS ip_allocations (
  id           CHAR(36)     NOT NULL,
  pool_id      CHAR(36)     NOT NULL,
  ip_address   VARCHAR(64)  NOT NULL,
  allocated_to VARCHAR(255) NULL,              -- resource id
  purpose      VARCHAR(255) NULL,              -- human label
  allocated_at DATETIME     NOT NULL DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (id),
  UNIQUE KEY uq_pool_ip (pool_id, ip_address),
  CONSTRAINT fk_alloc_pool FOREIGN KEY (pool_id) REFERENCES ip_pools(id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
