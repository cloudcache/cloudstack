use ldap3::{drive, LdapConnAsync, Scope, SearchEntry};
use serde::Serialize;
use tracing::debug;

use crate::{
    config::LdapConfig,
    error::{AppError, AppResult},
};

#[derive(Debug, Clone, Serialize)]
pub struct LdapUser {
    pub dn: String,
    pub uid: String,
    pub email: String,
    pub display_name: Option<String>,
    pub posix_uid: Option<u32>,
    pub posix_gid: Option<u32>,
    pub is_admin: bool,
}

pub struct LdapService {
    config: LdapConfig,
}

impl LdapService {
    pub fn new(config: LdapConfig) -> Self {
        Self { config }
    }

    pub async fn authenticate(&self, username: &str, password: &str) -> AppResult<LdapUser> {
        if password.is_empty() {
            return Err(AppError::Unauthorized("invalid credentials".into()));
        }

        // Step 1: admin bind + search user by uid OR email
        let (conn, mut ldap) = LdapConnAsync::new(&self.config.url)
            .await
            .map_err(|e| AppError::Ldap(e))?;
        drive!(conn);

        ldap.simple_bind(&self.config.bind_dn, &self.config.bind_password)
            .await?
            .success()
            .map_err(|e| AppError::Internal(format!("admin bind: {e}")))?;

        let search_base = format!("{},{}", self.config.user_ou, self.config.base_dn);
        let escaped = ldap3::ldap_escape(username);
        let filter = format!(
            "(&(objectClass=person)(|(uid={})(mail={})))",
            escaped, escaped
        );

        let (entries, _res) = ldap
            .search(
                &search_base,
                Scope::OneLevel,
                &filter,
                vec!["uid", "mail", "cn", "uidNumber", "gidNumber", "memberOf"],
            )
            .await?
            .success()
            .map_err(|e| AppError::Internal(format!("user search: {e}")))?;

        let entry = entries
            .into_iter()
            .next()
            .map(SearchEntry::construct)
            .ok_or_else(|| AppError::Unauthorized("invalid credentials".into()))?;

        // Step 2: new connection — bind as the found user to verify password
        let user_dn = entry.dn.clone();
        let (conn2, mut ldap2) = LdapConnAsync::new(&self.config.url)
            .await
            .map_err(|e| AppError::Ldap(e))?;
        drive!(conn2);

        let bind_result = ldap2.simple_bind(&user_dn, password).await?;
        if !bind_result.success().is_ok() {
            return Err(AppError::Unauthorized("invalid credentials".into()));
        }
        let _ = ldap2.unbind().await;
        debug!("LDAP bind success for {} (dn={})", username, user_dn);

        let get = |attr: &str| -> Option<String> { entry.attrs.get(attr)?.first().cloned() };

        let uid_str = get("uid").unwrap_or_else(|| username.to_string());
        let email = get("mail").unwrap_or_default();
        let display_name = get("cn");
        let posix_uid = get("uidNumber").and_then(|v| v.parse().ok());
        let posix_gid = get("gidNumber").and_then(|v| v.parse().ok());

        // Step 3: check admin group membership
        let admin_group_dn = format!(
            "cn=lldap_admin,{},{}",
            self.config.group_ou, self.config.base_dn
        );
        let is_admin = entry
            .attrs
            .get("memberOf")
            .map(|groups| {
                groups
                    .iter()
                    .any(|g| g.eq_ignore_ascii_case(&admin_group_dn))
            })
            .unwrap_or(false);

        ldap.unbind().await?;

        Ok(LdapUser {
            dn: user_dn,
            uid: uid_str,
            email,
            display_name,
            posix_uid,
            posix_gid,
            is_admin,
        })
    }

    pub async fn list_users(&self) -> AppResult<Vec<LdapUser>> {
        let (conn, mut ldap) = LdapConnAsync::new(&self.config.url)
            .await
            .map_err(AppError::Ldap)?;
        drive!(conn);

        ldap.simple_bind(&self.config.bind_dn, &self.config.bind_password)
            .await?
            .success()
            .map_err(|e| AppError::Internal(format!("admin bind: {e}")))?;

        let search_base = format!("{},{}", self.config.user_ou, self.config.base_dn);
        let (entries, _) = ldap
            .search(
                &search_base,
                Scope::OneLevel,
                "(&(objectClass=person)(uid=*))",
                vec!["uid", "mail", "cn", "uidNumber", "gidNumber", "memberOf"],
            )
            .await?
            .success()
            .map_err(|e| AppError::Internal(format!("user search: {e}")))?;

        let admin_group_dn = format!(
            "cn=lldap_admin,{},{}",
            self.config.group_ou, self.config.base_dn
        );

        let users = entries
            .into_iter()
            .filter_map(|entry| {
                let entry = SearchEntry::construct(entry);
                let get =
                    |attr: &str| -> Option<String> { entry.attrs.get(attr)?.first().cloned() };
                let uid = get("uid")?;
                let is_admin = entry
                    .attrs
                    .get("memberOf")
                    .map(|groups| {
                        groups
                            .iter()
                            .any(|g| g.eq_ignore_ascii_case(&admin_group_dn))
                    })
                    .unwrap_or(false);

                Some(LdapUser {
                    dn: entry.dn,
                    uid,
                    email: get("mail").unwrap_or_default(),
                    display_name: get("cn"),
                    posix_uid: get("uidNumber").and_then(|v| v.parse().ok()),
                    posix_gid: get("gidNumber").and_then(|v| v.parse().ok()),
                    is_admin,
                })
            })
            .collect();

        ldap.unbind().await?;
        Ok(users)
    }

    pub async fn rename_user(&self, current_dn: &str, new_uid: &str) -> AppResult<String> {
        if new_uid.trim().is_empty() {
            return Err(AppError::BadRequest("username is required".into()));
        }

        let (conn, mut ldap) = LdapConnAsync::new(&self.config.url)
            .await
            .map_err(AppError::Ldap)?;
        drive!(conn);

        ldap.simple_bind(&self.config.bind_dn, &self.config.bind_password)
            .await?
            .success()
            .map_err(|e| AppError::Internal(format!("admin bind: {e}")))?;

        let new_rdn = format!("uid={}", ldap3::dn_escape(new_uid));
        ldap.modifydn(current_dn, &new_rdn, true, None)
            .await?
            .success()
            .map_err(|e| AppError::Internal(format!("user rename: {e}")))?;

        ldap.unbind().await?;
        Ok(format!("{},{}", new_rdn, self.user_parent_dn()))
    }

    fn user_parent_dn(&self) -> String {
        format!("{},{}", self.config.user_ou, self.config.base_dn)
    }

    /// Check whether `username` is a member of LDAP group `cn=lldap_admin,...`.
    pub async fn is_global_admin(&self, user_dn: &str) -> AppResult<bool> {
        let (conn, mut ldap) = LdapConnAsync::new(&self.config.url)
            .await
            .map_err(|e| AppError::Ldap(e))?;
        drive!(conn);

        ldap.simple_bind(&self.config.bind_dn, &self.config.bind_password)
            .await?
            .success()
            .map_err(|e| AppError::Internal(format!("admin bind: {e}")))?;

        let group_base = format!("{},{}", self.config.group_ou, self.config.base_dn);
        let filter = format!("(&(cn=lldap_admin)(member={}))", ldap3::dn_escape(user_dn));

        let (entries, _) = ldap
            .search(&group_base, Scope::OneLevel, &filter, vec!["cn"])
            .await?
            .success()
            .map_err(|_| AppError::Internal("admin group search".into()))?;

        ldap.unbind().await?;
        Ok(!entries.is_empty())
    }
}
