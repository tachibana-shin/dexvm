//! Permission-based sandboxing, modeled after `d4rt`'s permission system
//! (and Deno's `--allow-*` flags).
//!
//! By default a [`Context`](crate::Context) denies every host capability.
//! The host grants specific capabilities through
//! [`Context::grant`](crate::Context::grant), checks them with
//! [`Context::has_permission`](crate::Context::has_permission) and
//! withdraws them with [`Context::revoke`](crate::Context::revoke).
//! Host-registered natives (see
//! [`Context::register_native`](crate::Context::register_native)) consult
//! the store with [`Vm::check_permission`](crate::Vm::check_permission)
//! before performing host side effects such as network I/O.
//! Built-in Java/Android/Okio shims enforce the same store and throw a Java
//! `SecurityException` on denial, so dex `try`/`catch` works as expected.
//!
//! Matching rules (mirroring `d4rt_rs`):
//! - `FilesystemPermission::Any` allows everything; `Read` allows any read
//!   access; `Write` any write access; `ReadPath(p)`/`WritePath(p)`/`Path(p)`
//!   allow access only under the given directory prefix.
//! - `NetworkPermission::Any` allows all network operations;
//!   `Connect(host)` allows connections to that host (with an optional
//!   `:port` suffix when the port must also match).
//! - `ProcessPermission::Any` allows running anything;
//!   `Command(cmd)` allows only that executable (exact path or bare name).
//! - `Permission::Env` allows reading host environment variables.

/// Filesystem capabilities (d4rt's `FilesystemPermission`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FilesystemPermission {
    /// All filesystem operations.
    Any,
    /// Read-only operations (including stat/list/exists).
    Read,
    /// Write operations.
    Write,
    /// Read operations under a specific directory prefix.
    ReadPath(String),
    /// Write operations under a specific directory prefix.
    WritePath(String),
    /// Read and write operations under a specific directory prefix.
    Path(String),
}

/// Network capabilities (d4rt's `NetworkPermission`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetworkPermission {
    /// All network operations.
    Any,
    /// Connect to a specific host, optionally `host:port`.
    Connect(String),
}

/// Process execution capabilities (d4rt's `ProcessPermission`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessPermission {
    /// Run any command.
    Any,
    /// Run only this executable (path or bare name).
    Command(String),
}

/// One grantable capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Permission {
    Filesystem(FilesystemPermission),
    Network(NetworkPermission),
    Process(ProcessPermission),
    /// Host environment variable reads (`System.getenv`).
    Env,
}

fn normalized_path(path: &str) -> std::path::PathBuf {
    use std::path::{Component, Path, PathBuf};

    let mut out = PathBuf::new();
    for component in Path::new(path).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                // Never let a relative `..` disappear past its logical root.
                // Keeping it makes a scoped grant fail closed.
                if out
                    .file_name()
                    .is_some_and(|name| name != std::ffi::OsStr::new(".."))
                {
                    out.pop();
                } else {
                    out.push(component.as_os_str());
                }
            }
            _ => out.push(component.as_os_str()),
        }
    }
    out
}

fn resolved_path(path: &str) -> std::path::PathBuf {
    let path = normalized_path(path);
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/"))
            .join(path)
    };
    let mut existing = absolute.clone();
    let mut missing = Vec::new();
    loop {
        if let Ok(real) = std::fs::canonicalize(&existing) {
            let mut result = real;
            for component in missing.into_iter().rev() {
                result.push(component);
            }
            return normalized_path(&result.to_string_lossy());
        }
        let Some(name) = existing.file_name().map(|v| v.to_owned()) else {
            return normalized_path(&absolute.to_string_lossy());
        };
        missing.push(name);
        if !existing.pop() {
            return normalized_path(&absolute.to_string_lossy());
        }
    }
}

fn path_under(prefix: &str, path: &str) -> bool {
    resolved_path(path).starts_with(resolved_path(prefix))
}

fn host_matches(allow: &str, host_port: &str) -> bool {
    fn explicit_port(value: &str) -> bool {
        let Some((host, port)) = value.rsplit_once(':') else {
            return false;
        };
        !host.is_empty()
            && port.chars().all(|c| c.is_ascii_digit())
            && (host.starts_with('[') || !host.contains(':'))
    }
    let allow = allow.to_ascii_lowercase();
    let host_port = host_port.to_ascii_lowercase();
    if explicit_port(&allow) {
        host_port == allow
    } else if host_port == allow {
        true
    } else {
        host_port
            .strip_prefix(&allow)
            .and_then(|rest| rest.strip_prefix(':'))
            .is_some_and(|port| port.chars().all(|c| c.is_ascii_digit()))
    }
}

impl Permission {
    /// Does `self` cover the capability `other`?
    pub fn covers(&self, other: &Permission) -> bool {
        match (self, other) {
            (Permission::Filesystem(a), Permission::Filesystem(b)) => match a {
                FilesystemPermission::Any => true,
                FilesystemPermission::Read => matches!(
                    b,
                    FilesystemPermission::Read | FilesystemPermission::ReadPath(_)
                ),
                FilesystemPermission::Write => matches!(
                    b,
                    FilesystemPermission::Write | FilesystemPermission::WritePath(_)
                ),
                FilesystemPermission::ReadPath(p) => match b {
                    FilesystemPermission::ReadPath(q) => path_under(p, q),
                    _ => false,
                },
                FilesystemPermission::WritePath(p) => match b {
                    FilesystemPermission::WritePath(q) => path_under(p, q),
                    _ => false,
                },
                FilesystemPermission::Path(p) => match b {
                    FilesystemPermission::ReadPath(q) | FilesystemPermission::WritePath(q) => {
                        path_under(p, q)
                    }
                    FilesystemPermission::Path(q) => path_under(p, q),
                    FilesystemPermission::Any
                    | FilesystemPermission::Read
                    | FilesystemPermission::Write => false,
                },
            },
            (Permission::Network(a), Permission::Network(b)) => match a {
                NetworkPermission::Any => true,
                NetworkPermission::Connect(h) => match b {
                    NetworkPermission::Connect(t) => host_matches(h, t),
                    NetworkPermission::Any => false,
                },
            },
            (Permission::Process(a), Permission::Process(b)) => match a {
                ProcessPermission::Any => true,
                ProcessPermission::Command(c) => {
                    matches!(b, ProcessPermission::Command(t) if t == c)
                }
            },
            (Permission::Env, Permission::Env) => true,
            _ => false,
        }
    }
}

/// The grant store owned by a [`Vm`](crate::Vm). Not exported as part of the
/// public API surface; use [`Context`](crate::Context) instead.
#[derive(Clone, Debug, Default)]
pub struct Permissions {
    grants: Vec<Permission>,
}

impl Permissions {
    pub fn new() -> Self {
        Permissions { grants: Vec::new() }
    }

    pub fn grant(&mut self, p: Permission) {
        if !self.grants.iter().any(|g| g.covers(&p)) {
            self.grants.push(p);
        }
    }

    /// Withdraws every grant that covers `p` (so revoking a specific
    /// capability also removes any broader grant that would allow it).
    pub fn revoke(&mut self, p: &Permission) {
        self.grants.retain(|g| !g.covers(p));
    }

    pub fn has(&self, p: &Permission) -> bool {
        self.grants.iter().any(|g| g.covers(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_deny_by_default() {
        let perms = Permissions::new();
        assert!(!perms.has(&Permission::Network(NetworkPermission::Any)));
        assert!(!perms.has(&Permission::Env));
    }

    #[test]
    fn any_covers_all() {
        let mut perms = Permissions::new();
        perms.grant(Permission::Network(NetworkPermission::Any));
        assert!(perms.has(&Permission::Network(NetworkPermission::Connect(
            "example.com".into()
        ))));
        perms.grant(Permission::Filesystem(FilesystemPermission::Any));
        assert!(
            perms.has(&Permission::Filesystem(FilesystemPermission::ReadPath(
                "/".into()
            )))
        );
        assert!(perms.has(&Permission::Filesystem(FilesystemPermission::Write)));
    }

    #[test]
    fn connect_host_matching() {
        let mut perms = Permissions::new();
        perms.grant(Permission::Network(NetworkPermission::Connect(
            "api.manga.example:443".into(),
        )));
        assert!(perms.has(&Permission::Network(NetworkPermission::Connect(
            "api.manga.example:443".into()
        ))));
        assert!(!perms.has(&Permission::Network(NetworkPermission::Connect(
            "api.manga.example:8443".into()
        ))));
        assert!(!perms.has(&Permission::Network(NetworkPermission::Connect(
            "evil.example:443".into()
        ))));

        let mut perms = Permissions::new();
        perms.grant(Permission::Network(NetworkPermission::Connect(
            "api.manga.example".into(),
        )));
        assert!(perms.has(&Permission::Network(NetworkPermission::Connect(
            "api.manga.example:443".into()
        ))));
        assert!(!perms.has(&Permission::Network(NetworkPermission::Connect(
            "api.manga.example.com:443".into()
        ))));
    }

    #[test]
    fn path_matching() {
        let mut perms = Permissions::new();
        perms.grant(Permission::Filesystem(FilesystemPermission::ReadPath(
            "/data".into(),
        )));
        assert!(
            perms.has(&Permission::Filesystem(FilesystemPermission::ReadPath(
                "/data/cache".into()
            )))
        );
        assert!(
            !perms.has(&Permission::Filesystem(FilesystemPermission::ReadPath(
                "/etc/passwd".into()
            )))
        );
        assert!(
            !perms.has(&Permission::Filesystem(FilesystemPermission::WritePath(
                "/data/cache".into()
            )))
        );
        perms.grant(Permission::Filesystem(FilesystemPermission::Path(
            "/data".into(),
        )));
        assert!(
            perms.has(&Permission::Filesystem(FilesystemPermission::WritePath(
                "/data/cache".into()
            )))
        );
        assert!(!perms.has(&Permission::Filesystem(FilesystemPermission::Read)));
        assert!(!perms.has(&Permission::Filesystem(FilesystemPermission::Write)));
        assert!(
            !perms.has(&Permission::Filesystem(FilesystemPermission::ReadPath(
                "/data/../etc/passwd".into()
            )))
        );
    }

    #[cfg(unix)]
    #[test]
    fn scoped_path_does_not_follow_symlink_outside_grant() {
        use std::os::unix::fs::symlink;
        use std::sync::atomic::{AtomicUsize, Ordering};

        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let base =
            std::env::temp_dir().join(format!("dexvm-permission-test-{}-{id}", std::process::id()));
        let allowed = base.join("allowed");
        let outside = base.join("outside");
        std::fs::create_dir_all(&allowed).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        symlink(&outside, allowed.join("escape")).unwrap();

        let grant = Permission::Filesystem(FilesystemPermission::ReadPath(
            allowed.to_string_lossy().into_owned(),
        ));
        let escaped = Permission::Filesystem(FilesystemPermission::ReadPath(
            allowed.join("escape/secret").to_string_lossy().into_owned(),
        ));
        assert!(!grant.covers(&escaped));
        std::fs::remove_file(allowed.join("escape")).unwrap();
        std::fs::remove_dir_all(base).unwrap();
    }
}
