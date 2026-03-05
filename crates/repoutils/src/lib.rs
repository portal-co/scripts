// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Repository utility helpers.
//!
//! Pure functions for parsing GitHub URLs and org/repo strings, plus thin
//! wrappers over `GitEnv` for common git-root queries.

use std::path::PathBuf;

use anyhow::Result;
use env_traits::GitEnv;

// ── OrgRepo ───────────────────────────────────────────────────────────────────

/// The result of parsing an `org/repo` string or a GitHub URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrgRepo {
    pub org:       String,
    pub repo:      String,
    /// `true` when the input was a URL or `org/repo` form (i.e. remote);
    /// `false` when the input was a bare repository name with no org.
    pub is_remote: bool,
}

/// Parse an input string as one of:
/// - `org/repo`
/// - `https://github.com/org/repo[.git]`
/// - `git@github.com:org/repo[.git]`
/// - bare repo name (org will be empty, `is_remote` false)
pub fn parse_org_repo(input: &str) -> OrgRepo {
    let input = input.trim_end_matches(".git");

    // HTTPS URL
    if let Some(rest) = input.strip_prefix("https://github.com/")
        .or_else(|| input.strip_prefix("http://github.com/"))
    {
        if let Some((org, repo)) = rest.split_once('/') {
            return OrgRepo { org: org.into(), repo: repo.into(), is_remote: true };
        }
    }

    // SSH URL: git@github.com:org/repo
    if let Some(rest) = input.strip_prefix("git@github.com:") {
        if let Some((org, repo)) = rest.split_once('/') {
            return OrgRepo { org: org.into(), repo: repo.into(), is_remote: true };
        }
    }

    // git:// or ssh:// URLs
    if input.starts_with("git://") || input.starts_with("ssh://") {
        if let Some(after_host) = input.splitn(4, '/').nth(3) {
            if let Some((org, repo)) = after_host.split_once('/') {
                return OrgRepo { org: org.into(), repo: repo.into(), is_remote: true };
            }
        }
    }

    // org/repo (slash-separated, no protocol)
    if let Some((org, repo)) = input.split_once('/') {
        return OrgRepo { org: org.into(), repo: repo.into(), is_remote: true };
    }

    // Bare name — treat as local path
    OrgRepo { org: String::new(), repo: input.into(), is_remote: false }
}

/// Return `true` if `s` looks like a git remote URL.
pub fn is_git_url(s: &str) -> bool {
    s.starts_with("git@")
        || s.starts_with("https://")
        || s.starts_with("http://")
        || s.starts_with("ssh://")
        || s.starts_with("git://")
}

/// Extract the repository name from a git URL (last path component,
/// `.git` suffix stripped).
pub fn repo_name_from_url(url: &str) -> &str {
    let url = url.trim_end_matches(".git");
    if let Some(pos) = url.rfind('/') {
        return &url[pos + 1..];
    }
    if let Some(pos) = url.rfind(':') {
        return &url[pos + 1..];
    }
    url
}

/// Return the name of the repository (the last component of `git rev-parse
/// --show-toplevel`).
pub fn current_repo_name<G: GitEnv>(git: &G) -> Result<PathBuf>
where
    G::Error: Send + Sync + 'static,
{
    let root = git.repo_root()?;
    Ok(std::path::Path::new(&root)
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(&root)))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_https_url() {
        let r = parse_org_repo("https://github.com/portal-co/scripts.git");
        assert_eq!(r.org, "portal-co");
        assert_eq!(r.repo, "scripts");
        assert!(r.is_remote);
    }

    #[test]
    fn parse_ssh_url() {
        let r = parse_org_repo("git@github.com:portal-co/scripts.git");
        assert_eq!(r.org, "portal-co");
        assert_eq!(r.repo, "scripts");
        assert!(r.is_remote);
    }

    #[test]
    fn parse_org_slash_repo() {
        let r = parse_org_repo("portal-co/scripts");
        assert_eq!(r.org, "portal-co");
        assert_eq!(r.repo, "scripts");
        assert!(r.is_remote);
    }

    #[test]
    fn parse_bare_name() {
        let r = parse_org_repo("scripts");
        assert_eq!(r.org, "");
        assert_eq!(r.repo, "scripts");
        assert!(!r.is_remote);
    }

    #[test]
    fn is_git_url_true() {
        assert!(is_git_url("git@github.com:a/b"));
        assert!(is_git_url("https://github.com/a/b"));
    }

    #[test]
    fn is_git_url_false() {
        assert!(!is_git_url("portal-co/scripts"));
    }

    #[test]
    fn repo_name_from_https_url() {
        assert_eq!(repo_name_from_url("https://github.com/org/my-repo.git"), "my-repo");
    }

    #[test]
    fn repo_name_from_ssh_url() {
        assert_eq!(repo_name_from_url("git@github.com:org/my-repo.git"), "my-repo");
    }
}
