-- 021_template_image_fields.sql
-- Promote container image from a generic `inputs[]` entry to first-class
-- columns on app_templates. Lets us search/filter by image, pin to a
-- specific registry (FK to image_registries), and pin a digest for
-- immutability. Backfills existing rows by extracting from inputs JSON.

-- image_registry_id: NULL means "use the system-default registry" (Docker Hub)
SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='app_templates'
     AND COLUMN_NAME='image_registry_id') = 0,
  'ALTER TABLE app_templates ADD COLUMN image_registry_id CHAR(36) NULL',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

-- image_repository: 'library/adminer', 'mysql', 'ghcr.io/owner/repo' (without tag)
SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='app_templates'
     AND COLUMN_NAME='image_repository') = 0,
  'ALTER TABLE app_templates ADD COLUMN image_repository VARCHAR(255) NOT NULL DEFAULT ''''',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

-- image_tag: '8.4', 'latest', 'v1.2.3'
SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='app_templates'
     AND COLUMN_NAME='image_tag') = 0,
  'ALTER TABLE app_templates ADD COLUMN image_tag VARCHAR(128) NOT NULL DEFAULT ''latest''',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

-- image_digest: optional 'sha256:...' for digest-pinned deploys
SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.COLUMNS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='app_templates'
     AND COLUMN_NAME='image_digest') = 0,
  'ALTER TABLE app_templates ADD COLUMN image_digest VARCHAR(128) NULL',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;

-- Backfill from inputs JSON: pull the `containerImageSource` entry's value,
-- split on ':' to separate repo and tag. Idempotent — runs only when both
-- image_repository is still empty AND inputs has the expected entry.
UPDATE app_templates
SET
  image_repository = SUBSTRING_INDEX(
    JSON_UNQUOTE(JSON_EXTRACT(inputs,
      CONCAT('$[', (
        SELECT JSON_SEARCH(inputs, 'one', 'containerImageSource', NULL, '$[*].key')
      ), '].value')
    )), ':', 1
  ),
  image_tag = COALESCE(
    NULLIF(SUBSTRING_INDEX(
      JSON_UNQUOTE(JSON_EXTRACT(inputs,
        CONCAT('$[', (
          SELECT JSON_SEARCH(inputs, 'one', 'containerImageSource', NULL, '$[*].key')
        ), '].value')
      )), ':', -1
    ), SUBSTRING_INDEX(
      JSON_UNQUOTE(JSON_EXTRACT(inputs,
        CONCAT('$[', (
          SELECT JSON_SEARCH(inputs, 'one', 'containerImageSource', NULL, '$[*].key')
        ), '].value')
      )), ':', 1
    )),
    'latest'
  )
WHERE image_repository = ''
  AND JSON_SEARCH(inputs, 'one', 'containerImageSource', NULL, '$[*].key') IS NOT NULL;

-- Searchable index
SET @_sql = IF(
  (SELECT COUNT(*) FROM information_schema.STATISTICS
   WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='app_templates'
     AND INDEX_NAME='idx_at_image_repo') = 0,
  'CREATE INDEX idx_at_image_repo ON app_templates(image_repository)',
  'SELECT 1');
PREPARE _stmt FROM @_sql; EXECUTE _stmt; DEALLOCATE PREPARE _stmt;
