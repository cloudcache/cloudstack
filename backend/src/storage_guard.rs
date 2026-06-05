/// Storage path sandbox — the "open_basedir" of QuickStack.
///
/// Every user-supplied host_path must reside under the platform's `storage_root`.
/// Admin-supplied paths (extra_volumes) may be anywhere but are blocked from
/// sensitive system directories.
use crate::error::{AppError, AppResult};

/// Paths that must never be mounted into a container, even by an admin.
const BLOCKED_PREFIXES: &[&str] = &[
    "/etc",
    "/proc",
    "/sys",
    "/dev",
    "/boot",
    "/root",
    "/var/run",
    "/run",
    "/usr/sbin",
    "/usr/lib/systemd",
];

/// Normalise a path string: resolve `..`, `.`, trailing slashes.
/// Returns an absolute, canonical-ish path string (no filesystem access needed).
fn normalize(path: &str) -> String {
    use std::path::{Component, PathBuf};
    let mut out = PathBuf::new();
    for comp in std::path::Path::new(path).components() {
        match comp {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out.to_string_lossy().to_string()
}

/// Check whether `path` starts with `prefix` after normalisation.
fn is_under(path: &str, prefix: &str) -> bool {
    let p = normalize(path);
    let base = normalize(prefix);
    // Exact match or path starts with base + '/'
    p == base || p.starts_with(&format!("{base}/"))
}

/// Validate a host_path intended for a **regular user** (managed volumes, etc.).
///
/// The path must be strictly under `storage_root`.
pub fn validate_user_path(host_path: &str, storage_root: &str) -> AppResult<()> {
    if !std::path::Path::new(host_path).is_absolute() {
        return Err(AppError::BadRequest("host_path must be absolute".into()));
    }
    if !is_under(host_path, storage_root) {
        return Err(AppError::Forbidden(format!(
            "host_path must be under storage root '{storage_root}'"
        )));
    }
    Ok(())
}

/// Validate a host_path intended for an **admin** (extra_volumes).
///
/// Allows paths outside storage_root but blocks known sensitive directories.
pub fn validate_admin_path(host_path: &str) -> AppResult<()> {
    if !std::path::Path::new(host_path).is_absolute() {
        return Err(AppError::BadRequest("host_path must be absolute".into()));
    }
    // Normalise to defeat ".." traversal
    let normalized = normalize(host_path);
    for prefix in BLOCKED_PREFIXES {
        if normalized == *prefix || normalized.starts_with(&format!("{prefix}/")) {
            return Err(AppError::BadRequest(format!(
                "host_path under '{prefix}' is not allowed"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_path_happy() {
        assert!(validate_user_path("/storage/projects/abc/vol1", "/storage").is_ok());
    }

    #[test]
    fn user_path_escape() {
        assert!(validate_user_path("/storage/../etc/shadow", "/storage").is_err());
    }

    #[test]
    fn user_path_outside_root() {
        assert!(validate_user_path("/tmp/data", "/storage").is_err());
    }

    #[test]
    fn admin_path_blocks_etc() {
        assert!(validate_admin_path("/etc/shadow").is_err());
    }

    #[test]
    fn admin_path_blocks_traversal() {
        assert!(validate_admin_path("/data/../etc/shadow").is_err());
    }

    #[test]
    fn admin_path_allows_storage() {
        assert!(validate_admin_path("/storage/custom/dir").is_ok());
    }

    #[test]
    fn normalize_handles_dots() {
        assert_eq!(normalize("/a/b/../c"), "/a/c");
        assert_eq!(normalize("/a/./b/c"), "/a/b/c");
        assert_eq!(normalize("/storage/../etc"), "/etc");
    }
}
