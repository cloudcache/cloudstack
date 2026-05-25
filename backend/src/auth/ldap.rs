use ldap3::{drive, LdapConnAsync, Scope, SearchEntry};
use tracing::{debug, warn};

use crate::{config::LdapConfig, error::{AppError, AppResult}};

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
        let (conn, mut ldap) = LdapConnAsync::new(&self.config.url)
            .await
            .map_err(|e| AppError::Ldap(e))?;
        drive!(conn);

        let user_dn = format!(
            "uid={},{},{}",
            ldap3::dn_escape(username),
            self.config.user_ou,
            self.config.base_dn
        );

        // Step 1: bind as the user to verify password
        let bind_result = ldap.simple_bind(&user_dn, password).await?;
        if !bind_result.success().is_ok() {
            return Err(AppError::Unauthorized("invalid credentials".into()));
        }
        debug!("LDAP bind success for {}", username);

        // Step 2: re-bind as admin to fetch attributes
        ldap.simple_bind(&self.config.bind_dn, &self.config.bind_password)
            .await?
            .success()
            .map_err(|e| AppError::Internal(format!("admin bind: {e}")))?;

        let search_base = format!("{},{}", self.config.user_ou, self.config.base_dn);
        let filter = self
            .config
            .user_filter
            .replace("{}", &ldap3::dn_escape(username));

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
            .ok_or_else(|| AppError::NotFound(format!("user {username} not found in LDAP")))?;

        let get = |attr: &str| -> Option<String> {
            entry.attrs.get(attr)?.first().cloned()
        };

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
            .map(|groups| groups.iter().any(|g| g.eq_ignore_ascii_case(&admin_group_dn)))
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
