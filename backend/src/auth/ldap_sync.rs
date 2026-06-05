use std::collections::BTreeSet;

use serde::Serialize;
use sqlx::{MySqlPool, Row};
use uuid::Uuid;

use crate::{
    auth::ldap::{LdapService, LdapUser},
    config::LdapConfig,
    error::{AppError, AppResult},
};

#[derive(Debug, Serialize)]
pub struct LdapSyncReport {
    pub scanned: usize,
    pub inserted: usize,
    pub updated: usize,
    pub skipped: usize,
    pub conflicts: Vec<LdapSyncConflict>,
}

#[derive(Debug, Serialize)]
pub struct LdapSyncConflict {
    pub ldap_dn: String,
    pub ldap_uid: String,
    pub ldap_email: String,
    pub reason: String,
    pub local_matches: Vec<LocalUserMatch>,
}

#[derive(Debug, Serialize)]
pub struct LocalUserMatch {
    pub id: String,
    pub username: String,
    pub email: String,
    pub ldap_dn: Option<String>,
}

struct LocalUser {
    id: String,
    username: String,
    email: String,
    ldap_dn: Option<String>,
}

pub async fn sync_ldap_users(db: &MySqlPool, config: &LdapConfig) -> AppResult<LdapSyncReport> {
    let ldap = LdapService::new(config.clone());
    let ldap_users = ldap.list_users().await?;
    let scanned = ldap_users.len();

    let mut report = LdapSyncReport {
        scanned,
        inserted: 0,
        updated: 0,
        skipped: 0,
        conflicts: Vec::new(),
    };

    for ldap_user in ldap_users {
        match sync_one_user(db, &ldap_user).await? {
            SyncAction::Inserted => report.inserted += 1,
            SyncAction::Updated => report.updated += 1,
            SyncAction::Conflict(conflict) => {
                report.skipped += 1;
                report.conflicts.push(conflict);
            }
        }
    }

    Ok(report)
}

enum SyncAction {
    Inserted,
    Updated,
    Conflict(LdapSyncConflict),
}

async fn sync_one_user(db: &MySqlPool, ldap_user: &LdapUser) -> AppResult<SyncAction> {
    if ldap_user.uid.trim().is_empty() || ldap_user.email.trim().is_empty() {
        return Ok(SyncAction::Conflict(conflict(
            ldap_user,
            "LDAP user must have both uid and mail",
            Vec::new(),
        )));
    }

    let matches = find_local_matches(db, ldap_user).await?;
    let distinct_ids: BTreeSet<&str> = matches.iter().map(|m| m.id.as_str()).collect();

    match distinct_ids.len() {
        0 => {
            insert_ldap_user(db, ldap_user).await?;
            Ok(SyncAction::Inserted)
        }
        1 => {
            let local = matches.first().expect("non-empty local matches");
            if local.ldap_dn.as_deref().is_none() {
                let uid_matches = local.username == ldap_user.uid;
                let email_matches = local.email.eq_ignore_ascii_case(&ldap_user.email);
                if !uid_matches || !email_matches {
                    return Ok(SyncAction::Conflict(conflict(
                        ldap_user,
                        "LDAP uid/mail must match the same local user before binding",
                        matches,
                    )));
                }
            }

            update_ldap_user(db, &local.id, ldap_user).await?;
            Ok(SyncAction::Updated)
        }
        _ => Ok(SyncAction::Conflict(conflict(
            ldap_user,
            "LDAP uid/mail/dn match multiple local users",
            matches,
        ))),
    }
}

async fn find_local_matches(db: &MySqlPool, ldap_user: &LdapUser) -> AppResult<Vec<LocalUser>> {
    let rows = sqlx::query(
        r#"SELECT id, username, email, ldap_dn
           FROM users
           WHERE ldap_dn = ? OR username = ? OR email = ?"#,
    )
    .bind(&ldap_user.dn)
    .bind(&ldap_user.uid)
    .bind(&ldap_user.email)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| LocalUser {
            id: row.get("id"),
            username: row.get("username"),
            email: row.get("email"),
            ldap_dn: row.get("ldap_dn"),
        })
        .collect())
}

async fn insert_ldap_user(db: &MySqlPool, ldap_user: &LdapUser) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO users
           (id, username, email, display_name, ldap_dn, ldap_uid, ldap_gid, is_global_admin)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&ldap_user.uid)
    .bind(&ldap_user.email)
    .bind(&ldap_user.display_name)
    .bind(&ldap_user.dn)
    .bind(ldap_user.posix_uid)
    .bind(ldap_user.posix_gid)
    .bind(ldap_user.is_admin as i8)
    .execute(db)
    .await
    .map_err(map_unique_violation)?;
    Ok(())
}

async fn update_ldap_user(db: &MySqlPool, user_id: &str, ldap_user: &LdapUser) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE users
           SET username = ?, email = ?, display_name = ?, ldap_dn = ?,
               ldap_uid = ?, ldap_gid = ?,
               is_global_admin = CASE WHEN ? = 1 THEN 1 ELSE is_global_admin END,
               updated_at = NOW()
           WHERE id = ?"#,
    )
    .bind(&ldap_user.uid)
    .bind(&ldap_user.email)
    .bind(&ldap_user.display_name)
    .bind(&ldap_user.dn)
    .bind(ldap_user.posix_uid)
    .bind(ldap_user.posix_gid)
    .bind(ldap_user.is_admin as i8)
    .bind(user_id)
    .execute(db)
    .await
    .map_err(map_unique_violation)?;
    Ok(())
}

fn map_unique_violation(err: sqlx::Error) -> AppError {
    match err {
        sqlx::Error::Database(ref de) if de.is_unique_violation() => AppError::Conflict(
            "LDAP user conflicts with an existing local username or email".into(),
        ),
        other => AppError::Database(other),
    }
}

fn conflict(ldap_user: &LdapUser, reason: &str, matches: Vec<LocalUser>) -> LdapSyncConflict {
    LdapSyncConflict {
        ldap_dn: ldap_user.dn.clone(),
        ldap_uid: ldap_user.uid.clone(),
        ldap_email: ldap_user.email.clone(),
        reason: reason.to_string(),
        local_matches: matches
            .into_iter()
            .map(|m| LocalUserMatch {
                id: m.id,
                username: m.username,
                email: m.email,
                ldap_dn: m.ldap_dn,
            })
            .collect(),
    }
}
