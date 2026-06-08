use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, exit};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let op = args.get(1).map(String::as_str).unwrap_or("");
    let repo = std::env::current_dir().expect("failed to get cwd");
    let mut seen = HashSet::new();
    let result = match op {
        "push" => push_recurse(&repo, &mut seen),
        "pull" => pull_recurse(&repo, &mut seen),
        _ => {
            eprintln!("usage: git-recurse push|pull");
            exit(2);
        }
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

fn push_recurse(repo: &Path, seen: &mut HashSet<PathBuf>) -> anyhow::Result<()> {
    let canon = repo.canonicalize()?;
    if !seen.insert(canon.clone()) {
        return Ok(());
    }
    for remote in git_remotes(&canon)? {
        let status = Command::new("git")
            .args(["-C", &canon.to_string_lossy(), "push", &remote])
            .status()?;
        if !status.success() {
            anyhow::bail!("git push {} failed in {}", remote, canon.display());
        }
        let url = remote_url(&canon, &remote)?;
        if let Some(local) = local_path(&canon, &url) {
            push_recurse(&local, seen)?;
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
