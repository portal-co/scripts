use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, exit};

use clap::Parser;

#[derive(Parser, Debug)]
#[command(about = "Recursively push/pull across chained local git remotes")]
struct Cli {
    #[command(subcommand)]
    op: Op,
}

#[derive(clap::Subcommand, Debug)]
enum Op {
    /// Push to every remote, recursing into local-path remotes.
    Push {
        /// If `git push` to a GitHub remote fails, open (or update) a PR instead of failing.
        #[arg(long, env = "GIT_RECURSE_PR_ON_FAIL")]
        pr_on_fail: bool,
    },
    /// Pull from every remote, recursing into local-path remotes first.
    Pull,
}

fn main() {
    let cli = Cli::parse();
    let repo = std::env::current_dir().expect("failed to get cwd");
    let mut seen = HashSet::new();
    let result = match cli.op {
        Op::Push { pr_on_fail } => push_recurse(&repo, &mut seen, pr_on_fail),
        Op::Pull => pull_recurse(&repo, &mut seen),
    };
    if let Err(e) = result {
        eprintln!("git-recurse: {e}");
        exit(1);
    }
}

fn git_remotes(repo: &Path) -> anyhow::Result<Vec<String>> {
    let out = Command::new("git")
        .args(["-C", &repo.to_string_lossy(), "remote"])
        .output()?;
    if !out.status.success() || out.stdout.is_empty() {
        return Ok(vec![]);
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect())
}

fn remote_url(repo: &Path, remote: &str) -> anyhow::Result<String> {
    let out = Command::new("git")
        .args(["-C", &repo.to_string_lossy(), "remote", "get-url", remote])
        .output()?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn local_path(repo: &Path, url: &str) -> Option<PathBuf> {
    let raw = if let Some(rest) = url.strip_prefix("file://") {
        if !rest.starts_with('/') {
            return None; // file://host/path — not a local path
        }
        rest
    } else if url.starts_with('/') || url.starts_with("./") || url.starts_with("../") {
        url
    } else {
        return None;
    };
    let p = if raw.starts_with('/') {
        PathBuf::from(raw)
    } else {
        repo.join(raw)
    };
    p.exists().then_some(p)
}

/// Only `github.com` remotes are eligible for the PR fallback (no GitHub
/// Enterprise / self-hosted support — out of scope).
fn is_github_remote(url: &str) -> bool {
    url.contains("github.com")
}

/// Run `gh <args>`, returning trimmed stdout on success or stderr as Err.
fn gh_capture(args: &[&str]) -> anyhow::Result<String> {
    let out = Command::new("gh").args(args).output()?;
    if !out.status.success() {
        anyhow::bail!(
            "gh {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn git_capture(repo: &Path, args: &[&str]) -> anyhow::Result<String> {
    let repo_str = repo.to_string_lossy();
    let mut full_args = vec!["-C", repo_str.as_ref()];
    full_args.extend_from_slice(args);
    let out = Command::new("git").args(&full_args).output()?;
    if !out.status.success() {
        anyhow::bail!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Parse the sequence number `N` out of an `auto-pr/<branch>-N` ref name.
fn parse_auto_pr_seq(head_ref: &str, branch: &str) -> Option<u64> {
    head_ref.strip_prefix(&format!("auto-pr/{branch}-"))?.parse().ok()
}

struct PrCandidate {
    head_ref: String,
    head_sha: String,
    url: String,
}

/// Open PRs on `slug` whose head ref matches `auto-pr/<branch>-N`, sorted by
/// ascending sequence number (oldest/lowest-numbered first).
fn list_open_auto_pr_candidates(slug: &str, branch: &str) -> anyhow::Result<Vec<PrCandidate>> {
    let out = gh_capture(&[
        "pr",
        "list",
        "--repo",
        slug,
        "--state",
        "open",
        "--json",
        "headRefName,headRefOid,url",
    ])?;
    let json: serde_json::Value = serde_json::from_str(&out)?;
    let mut candidates: Vec<(u64, PrCandidate)> = json
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|pr| {
            let head_ref = pr["headRefName"].as_str()?.to_string();
            let seq = parse_auto_pr_seq(&head_ref, branch)?;
            Some((
                seq,
                PrCandidate {
                    head_ref,
                    head_sha: pr["headRefOid"].as_str()?.to_string(),
                    url: pr["url"].as_str()?.to_string(),
                },
            ))
        })
        .collect();
    candidates.sort_by_key(|(seq, _)| *seq);
    Ok(candidates.into_iter().map(|(_, c)| c).collect())
}

/// True if `ancestor_sha` is an ancestor of (or equal to) local HEAD — i.e.
/// HEAD already contains every commit that PR branch has.
fn is_ancestor(repo: &Path, ancestor_sha: &str) -> bool {
    Command::new("git")
        .args([
            "-C",
            &repo.to_string_lossy(),
            "merge-base",
            "--is-ancestor",
            ancestor_sha,
            "HEAD",
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Next unused `auto-pr/<branch>-N` sequence number, scanning existing
/// remote branches (not just PRs) so a closed/merged PR's branch name is
/// never reused.
fn next_sequence(repo: &Path, remote: &str, branch: &str) -> anyhow::Result<u64> {
    let out = git_capture(
        repo,
        &[
            "ls-remote",
            "--heads",
            remote,
            &format!("refs/heads/auto-pr/{branch}-*"),
        ],
    )?;
    let max_seq = out
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .filter_map(|r| r.strip_prefix("refs/heads/"))
        .filter_map(|head_ref| parse_auto_pr_seq(head_ref, branch))
        .max()
        .unwrap_or(0);
    Ok(max_seq + 1)
}

/// Push the current commit onto a reusable or new `auto-pr/<branch>-N`
/// branch and return the URL of the (possibly pre-existing) PR targeting
/// `branch` (the same branch the original `git push` was rejected on).
fn open_pr_fallback(repo: &Path, remote: &str, url: &str) -> anyhow::Result<String> {
    let org_repo = repoutils::parse_org_repo(url);
    if !org_repo.is_remote || org_repo.org.is_empty() {
        anyhow::bail!("could not parse org/repo from remote URL: {url}");
    }
    let slug = format!("{}/{}", org_repo.org, org_repo.repo);
    let current_branch = git_capture(repo, &["rev-parse", "--abbrev-ref", "HEAD"])?;

    // Reuse the first open auto-pr/<branch>-N PR whose head is already an
    // ancestor of our current HEAD (fast-forwardable) instead of opening a
    // new PR for every failed push.
    for candidate in list_open_auto_pr_candidates(&slug, &current_branch)? {
        if is_ancestor(repo, &candidate.head_sha) {
            git_capture(
                repo,
                &["push", remote, &format!("HEAD:refs/heads/{}", candidate.head_ref)],
            )?;
            return Ok(candidate.url);
        }
    }

    // No reusable PR — open a new one on the next sequential branch name,
    // targeting the same branch the original push was rejected on.
    let seq = next_sequence(repo, remote, &current_branch)?;
    let head_branch = format!("auto-pr/{current_branch}-{seq}");
    git_capture(repo, &["push", remote, &format!("HEAD:refs/heads/{head_branch}")])?;
    gh_capture(&[
        "pr",
        "create",
        "--repo",
        &slug,
        "--head",
        &head_branch,
        "--base",
        &current_branch,
        "--fill",
    ])
}

fn push_recurse(repo: &Path, seen: &mut HashSet<PathBuf>, pr_on_fail: bool) -> anyhow::Result<()> {
    let canon = repo.canonicalize()?;
    if !seen.insert(canon.clone()) {
        return Ok(());
    }
    for remote in git_remotes(&canon)? {
        let status = Command::new("git")
            .args(["-C", &canon.to_string_lossy(), "push", &remote])
            .status()?;
        if !status.success() {
            let push_err = format!("git push {} failed in {}", remote, canon.display());
            if pr_on_fail {
                let url = remote_url(&canon, &remote)?;
                if is_github_remote(&url) {
                    match open_pr_fallback(&canon, &remote, &url) {
                        Ok(pr_url) => {
                            eprintln!(
                                "git-recurse: push to {remote} failed; opened/updated PR instead: {pr_url}"
                            );
                            continue;
                        }
                        Err(pr_err) => {
                            anyhow::bail!("{push_err}; PR fallback also failed: {pr_err}")
                        }
                    }
                }
            }
            anyhow::bail!(push_err);
        }
        let url = remote_url(&canon, &remote)?;
        if let Some(local) = local_path(&canon, &url) {
            push_recurse(&local, seen, pr_on_fail)?;
        }
    }
    Ok(())
}

fn pull_recurse(repo: &Path, seen: &mut HashSet<PathBuf>) -> anyhow::Result<()> {
    let canon = repo.canonicalize()?;
    if !seen.insert(canon.clone()) {
        return Ok(());
    }
    for remote in git_remotes(&canon)? {
        let url = remote_url(&canon, &remote)?;
        if let Some(local) = local_path(&canon, &url) {
            pull_recurse(&local, seen)?; // update local remote from its upstreams before pulling
        }
        let status = Command::new("git")
            .args(["-C", &canon.to_string_lossy(), "pull", "--no-rebase", &remote])
            .status()?;
        if !status.success() {
            anyhow::bail!("git pull {} failed in {}", remote, canon.display());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_auto_pr_seq_matches_branch_prefix() {
        assert_eq!(parse_auto_pr_seq("auto-pr/main-3", "main"), Some(3));
        assert_eq!(parse_auto_pr_seq("auto-pr/main-abc", "main"), None);
        assert_eq!(parse_auto_pr_seq("feature/foo", "main"), None);
        assert_eq!(parse_auto_pr_seq("auto-pr/other-3", "main"), None);
    }

    #[test]
    fn is_github_remote_matches_https_and_ssh() {
        assert!(is_github_remote("https://github.com/org/repo.git"));
        assert!(is_github_remote("git@github.com:org/repo.git"));
        assert!(!is_github_remote("https://gitlab.com/org/repo.git"));
        assert!(!is_github_remote("/local/path/to/repo"));
    }
}
