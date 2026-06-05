//! App-template seed loader.
//!
//! Phase 1 of the templates-v2 migration: the existing TS-hardcoded catalog is
//! shipped as JSON blobs (under `templates/seed/`) and upserted into the
//! `app_templates` table on first startup. From there, templates live in the
//! DB and admins maintain them via the REST API (see `api::templates`).
//!
//! `requirements` is left as an empty array — Phase 2 will populate it and
//! drive managed-service binding during deploy.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{error::AppResult, state::AppState};

#[derive(Serialize, Deserialize)]
struct SeedEntry {
    slug: String,
    name: String,
    icon_url: Option<String>,
    category: String,
    #[serde(default)]
    image_repository: String,
    #[serde(default = "default_latest")]
    image_tag: String,
    spec: serde_json::Value,
    inputs: serde_json::Value,
}

fn default_latest() -> String { "latest".to_string() }

/// Every template JSON shipped with the binary.
/// Add new lines here as new seed files land in `templates/seed/`.
const SEED_FILES: &[(&str, &str)] = &[
    ("adminer",       include_str!("seed/adminer.json")),
    ("apache-tika",   include_str!("seed/apache-tika.json")),
    ("chisel-tunnel", include_str!("seed/chisel-tunnel.json")),
    ("docmost",       include_str!("seed/docmost.json")),
    ("draw-io",       include_str!("seed/draw-io.json")),
    ("duplicati",     include_str!("seed/duplicati.json")),
    ("gitea",         include_str!("seed/gitea.json")),
    ("libredesk",     include_str!("seed/libredesk.json")),
    ("mariadb",       include_str!("seed/mariadb.json")),
    ("minio",         include_str!("seed/minio.json")),
    ("mongodb",       include_str!("seed/mongodb.json")),
    ("mysql",         include_str!("seed/mysql.json")),
    ("n8n",           include_str!("seed/n8n.json")),
    ("nextcloud",     include_str!("seed/nextcloud.json")),
    ("nginx",         include_str!("seed/nginx.json")),
    ("open-webui",    include_str!("seed/open-webui.json")),
    ("postgresql",    include_str!("seed/postgresql.json")),
    ("redis",         include_str!("seed/redis.json")),
    ("uptime-kuma",   include_str!("seed/uptime-kuma.json")),
    ("wordpress",     include_str!("seed/wordpress.json")),
];

/// Insert any seed templates that aren't already in the DB. Existing rows
/// (matched by slug) are left untouched so admin edits stick across restarts.
pub async fn seed_if_missing(state: &AppState) -> AppResult<()> {
    let existing: Vec<String> = sqlx::query_scalar(
        "SELECT slug FROM app_templates WHERE visibility = 'PUBLIC'",
    )
    .fetch_all(&state.db)
    .await?;
    let existing: std::collections::HashSet<String> = existing.into_iter().collect();

    let mut inserted = 0;
    for (_slug, json) in SEED_FILES {
        let entry: SeedEntry = match serde_json::from_str(json) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("template seed parse error: {e}");
                continue;
            }
        };
        if existing.contains(&entry.slug) {
            continue;
        }

        let id = Uuid::new_v4().to_string();
        let spec_str = entry.spec.to_string();
        let inputs_str = entry.inputs.to_string();
        let empty_arr = "[]".to_string();

        sqlx::query(
            "INSERT INTO app_templates \
                (id, slug, name, icon_url, category, visibility, \
                 image_repository, image_tag, \
                 spec, requirements, inputs) \
             VALUES (?, ?, ?, ?, ?, 'PUBLIC', ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&entry.slug)
        .bind(&entry.name)
        .bind(&entry.icon_url)
        .bind(&entry.category)
        .bind(&entry.image_repository)
        .bind(&entry.image_tag)
        .bind(&spec_str)
        .bind(&empty_arr)
        .bind(&inputs_str)
        .execute(&state.db)
        .await?;
        inserted += 1;
    }

    if inserted > 0 {
        tracing::info!(inserted, "seeded app templates");
    }
    Ok(())
}
