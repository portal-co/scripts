// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Generate `.cargo/config.toml` `[patch]` sections from local repo checkouts.
//!
//! # Usage
//!
//! ```text
//! gen-patch [OPTIONS] [REPOS_DIR]
//! ```
//!
//! `REPOS_DIR` is the directory whose immediate subdirectories are local git
//! checkouts.  It defaults to the current working directory.
//!
//! For each subdirectory that has both a `Cargo.toml` and a git remote named
//! `origin`, the tool:
//!
//! 1. Reads the remote URL via `git remote get-url origin`.
//! 2. Parses `Cargo.toml` to find workspace members (or treats the crate
//!    itself as the sole member when there is no `[workspace]` section).
//! 3. Reads each member's `Cargo.toml` to get its `[package].name`.
//! 4. Emits a `[patch.'<url>']` TOML section mapping each name to its path
//!    relative to `REPOS_DIR`.
//!
//! The output is written to `<REPOS_DIR>/.cargo/config.toml` by default (the
//! directory is created if it does not exist).  Use `--output` to redirect.
//!
//! # Why relative paths work
//!
//! Cargo resolves relative paths in `[patch]` entries found in
//! `.cargo/config.toml` relative to the parent of the `.cargo/` directory.
//! When the output is `<REPOS_DIR>/.cargo/config.toml`, that parent is
//! `REPOS_DIR` itself, so every generated path `<repo>/<member>` resolves
//! correctly regardless of which project inside `REPOS_DIR` is being built.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use clap::Parser;
use toml::Value;

// ── CLI ───────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "gen-patch",
    about = "Generate .cargo/config.toml [patch] sections from local repo checkouts"
)]
struct Cli {
    /// Directory whose immediate subdirectories are the local repo checkouts.
    ///
    /// Paths in the generated [patch] sections are relative to this directory.
    /// Defaults to the current working directory.
    #[arg(default_value = ".")]
    repos_dir: PathBuf,

    /// Where to write the generated config.toml.
    ///
    /// Defaults to <REPOS_DIR>/.cargo/config.toml.
    #[arg(long)]
    output: Option<PathBuf>,
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    if let Err(e) = run() {
        eprintln!("gen-patch: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    let repos_dir = cli
        .repos_dir
        .canonicalize()
        .context("failed to canonicalize REPOS_DIR")?;

    let output = cli
        .output
        .unwrap_or_else(|| repos_dir.join(".cargo").join("config.toml"));

    // Collect all patch sections, one per repo that qualifies.
    let mut sections: Vec<PatchSection> = Vec::new();

    let mut repo_dirs: Vec<_> = fs::read_dir(&repos_dir)
        .context("failed to read REPOS_DIR")?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    repo_dirs.sort();

    for repo_path in repo_dirs {
        match process_repo(&repo_path, &repos_dir) {
            Ok(Some(section)) => sections.push(section),
            Ok(None) => {}
            Err(e) => {
                let name = repo_path.file_name().unwrap_or_default().to_string_lossy();
                eprintln!("gen-patch: warning: skipping {name}: {e:#}");
            }
        }
    }

    // Write output.
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }

    let content = render(&sections);
    fs::write(&output, &content)
        .with_context(|| format!("writing {}", output.display()))?;

    eprintln!(
        "gen-patch: wrote {} patch section(s) to {}",
        sections.len(),
        output.display()
    );
    Ok(())
}

// ── Per-repo logic ────────────────────────────────────────────────────────────

/// One `[patch.'<url>']` section.
struct PatchSection {
    url: String,
    /// `(package_name, path_relative_to_repos_dir)`
    entries: Vec<(String, String)>,
}

/// Try to build a [`PatchSection`] for a single repo directory.
///
/// Returns `Ok(None)` when the directory should be silently skipped (no
/// `Cargo.toml`, no git remote).  Returns `Err` for unexpected problems.
fn process_repo(repo_path: &Path, repos_dir: &Path) -> Result<Option<PatchSection>> {
    // Must have a Cargo.toml.
    let root_toml_path = repo_path.join("Cargo.toml");
    if !root_toml_path.exists() {
        return Ok(None);
    }

    // Must have a git remote named `origin`.
    let url = match git_remote_url(repo_path) {
        Some(u) => u,
        None => return Ok(None),
    };

    // Parse the root Cargo.toml.
    let root_toml = read_toml(&root_toml_path)
        .with_context(|| format!("parsing {}", root_toml_path.display()))?;

    let has_workspace = root_toml.get("workspace").is_some();

    let mut entries: Vec<(String, String)> = Vec::new();

    if has_workspace {
        let members = workspace_members(&root_toml);

        for member in &members {
            let (member_toml_path, rel_path) = if member == "." {
                // Workspace root doubles as a package; path is the repo dir itself.
                (root_toml_path.clone(), repo_rel_path(repo_path, repos_dir))
            } else {
                let abs = repo_path.join(member);
                let rel = path_to_string(
                    abs.strip_prefix(repos_dir)
                        .context("member path is outside REPOS_DIR")?,
                );
                (abs.join("Cargo.toml"), rel)
            };

            match package_name_from_path(&member_toml_path) {
                Some(name) => entries.push((name, rel_path)),
                None => {
                    // No [package] section — virtual manifest, skip.
                }
            }
        }

        // If the root has [package] but "." is not explicitly listed as a
        // member (some repos omit it), include the root crate anyway.
        if !members.iter().any(|m| m == ".") {
            if let Some(name) = package_name_from_toml(&root_toml) {
                entries.push((name, repo_rel_path(repo_path, repos_dir)));
            }
        }
    } else {
        // Single-crate repo.
        if let Some(name) = package_name_from_toml(&root_toml) {
            entries.push((name, repo_rel_path(repo_path, repos_dir)));
        }
    }

    if entries.is_empty() {
        return Ok(None);
    }

    Ok(Some(PatchSection { url, entries }))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Run `git -C <dir> remote get-url origin` and return the trimmed output, or
/// `None` if the command fails (not a git repo, or no remote named `origin`).
fn git_remote_url(dir: &Path) -> Option<String> {
    let out = Command::new("git")
        .args([
            "-C",
            dir.to_str()?,
            "remote",
            "get-url",
            "origin",
        ])
        .output()
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

/// Read and parse a TOML file into a [`Value`].
fn read_toml(path: &Path) -> Result<Value> {
    let s = fs::read_to_string(path)
        .with_context(|| format!("reading {}", path.display()))?;
    s.parse::<Value>()
        .with_context(|| format!("parsing TOML in {}", path.display()))
}

/// Extract `[workspace].members` as a `Vec<String>`.
fn workspace_members(toml: &Value) -> Vec<String> {
    toml.get("workspace")
        .and_then(|ws| ws.get("members"))
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Read a Cargo.toml at `path` and return its `[package].name`, if present.
fn package_name_from_path(path: &Path) -> Option<String> {
    let toml = read_toml(path)
        .map_err(|e| eprintln!("gen-patch: warning: {e:#}"))
        .ok()?;
    package_name_from_toml(&toml)
}

/// Extract `[package].name` from an already-parsed [`Value`].
///
/// Returns `None` when the key is absent or is not a plain string (e.g. when
/// `name.workspace = true` is used, which is unusual but syntactically legal).
fn package_name_from_toml(toml: &Value) -> Option<String> {
    toml.get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .map(String::from)
}

/// Compute the path of `repo_path` relative to `repos_dir` as a forward-slash
/// string.
fn repo_rel_path(repo_path: &Path, repos_dir: &Path) -> String {
    repo_path
        .strip_prefix(repos_dir)
        .map(path_to_string)
        .unwrap_or_else(|_| repo_path.to_string_lossy().into_owned())
}

/// Convert a [`Path`] to a forward-slash string (important on Windows).
fn path_to_string(p: &Path) -> String {
    p.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

// ── Rendering ─────────────────────────────────────────────────────────────────

/// Render all patch sections as a TOML string.
fn render(sections: &[PatchSection]) -> String {
    let mut out = String::from(
        "# Generated by gen-patch — do not edit by hand.\n\
         # Re-run `gen-patch` to refresh after adding or removing repos.\n",
    );

    for section in sections {
        out.push('\n');
        out.push_str(&format!("[patch.'{}']\n", section.url));

        // Align the ` = ` separators within each section.
        let max_len = section
            .entries
            .iter()
            .map(|(name, _)| name.len())
            .max()
            .unwrap_or(0);

        for (name, path) in &section.entries {
            let pad = " ".repeat(max_len - name.len());
            out.push_str(&format!("{name}{pad} = {{ path = \"{path}\" }}\n"));
        }
    }

    out
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_to_string_forward_slashes() {
        let p = Path::new("foo").join("bar").join("baz");
        assert_eq!(path_to_string(&p), "foo/bar/baz");
    }

    #[test]
    fn package_name_from_toml_plain() {
        let v: Value = "[package]\nname = \"my-crate\"\n"
            .parse()
            .unwrap();
        assert_eq!(package_name_from_toml(&v), Some("my-crate".into()));
    }

    #[test]
    fn package_name_from_toml_missing() {
        let v: Value = "[workspace]\nmembers = []\n".parse().unwrap();
        assert_eq!(package_name_from_toml(&v), None);
    }

    #[test]
    fn package_name_inherited_returns_none() {
        // `name.workspace = true` — unusual but legal TOML; we can't resolve
        // it without the workspace root, so we return None rather than panic.
        let v: Value = "[package]\n[package.name]\nworkspace = true\n"
            .parse()
            .unwrap();
        assert_eq!(package_name_from_toml(&v), None);
    }

    #[test]
    fn workspace_members_extracted() {
        let v: Value = "[workspace]\nmembers = [\"crates/a\", \"crates/b\"]\n"
            .parse()
            .unwrap();
        assert_eq!(
            workspace_members(&v),
            vec!["crates/a".to_string(), "crates/b".to_string()]
        );
    }

    #[test]
    fn render_aligns_entries() {
        let sections = vec![PatchSection {
            url: "https://github.com/org/repo.git".into(),
            entries: vec![
                ("short".into(), "repo".into()),
                ("much-longer-name".into(), "repo/crates/x".into()),
            ],
        }];
        let out = render(&sections);
        assert!(out.contains("[patch.'https://github.com/org/repo.git']"));
        // Both lines should have the same number of chars before ` = `.
        let lines: Vec<&str> = out.lines().filter(|l| l.contains(" = {")).collect();
        assert_eq!(lines.len(), 2);
        let col0 = lines[0].find(" = ").unwrap();
        let col1 = lines[1].find(" = ").unwrap();
        assert_eq!(col0, col1);
    }

    #[test]
    fn process_repo_no_cargo_toml() {
        let dir = tempfile::tempdir().unwrap();
        let result = process_repo(dir.path(), dir.path()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn process_repo_virtual_workspace_no_package() {
        // A virtual workspace manifest (only [workspace], no [package]) with
        // no members should produce no patch entries.
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = []\n",
        )
        .unwrap();
        // No git repo → git_remote_url returns None → Ok(None).
        let result = process_repo(dir.path(), dir.path()).unwrap();
        assert!(result.is_none());
    }
}
