// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Discover directories under the current working tree for batch git/cargo scripts.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

/// Lockfile basenames treated as the only allowed dirty paths (matches `tools/git/gen.py`).
pub const LOCKFILE_BASENAMES: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
];

/// Filter flags for directory discovery.
#[derive(Debug, Clone, Copy, Default)]
pub struct Filters {
    pub git: bool,
    pub cargo: bool,
    pub clean_lockfiles_only: bool,
}

/// Collect relative directory paths (depths 1..=`max_depth`) matching `filters`.
pub fn list_directories(root: &Path, max_depth: u32, filters: Filters) -> Result<Vec<PathBuf>> {
    let root = root
        .canonicalize()
        .with_context(|| format!("canonicalize {}", root.display()))?;
    let mut out = Vec::new();
    walk(
        &root,
        &root,
        1,
        max_depth,
        filters,
        &mut out,
    )?;
    out.sort();
    Ok(out)
}

fn walk(
    root: &Path,
    dir: &Path,
    depth: u32,
    max_depth: u32,
    filters: Filters,
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    if depth > max_depth {
        return Ok(());
    }

    let mut children: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("read_dir {}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    children.sort();

    for child in children {
        if child.file_name() == Some(OsStr::new(".git")) {
            continue;
        }

        if matches_filters(&child, filters)? {
            let rel = child
                .strip_prefix(root)
                .with_context(|| format!("strip_prefix {}", child.display()))?;
            out.push(format_dir_line(rel));
        }

        if depth < max_depth {
            walk(root, &child, depth + 1, max_depth, filters, out)?;
        }
    }

    Ok(())
}

/// Emit a relative path with trailing `/` (matches `ls -d */` lines for forfiles).
fn format_dir_line(rel: &Path) -> PathBuf {
    let mut s = rel.to_string_lossy().into_owned();
    if !s.ends_with('/') {
        s.push('/');
    }
    PathBuf::from(s)
}

fn matches_filters(dir: &Path, filters: Filters) -> Result<bool> {
    if filters.git && !is_git_work_tree(dir)? {
        return Ok(false);
    }
    if filters.cargo && !dir.join("Cargo.toml").is_file() {
        return Ok(false);
    }
    if filters.clean_lockfiles_only && !is_clean_except_lockfiles(dir)? {
        return Ok(false);
    }
    Ok(true)
}

fn is_git_work_tree(dir: &Path) -> Result<bool> {
    let ok = Command::new("git")
        .args(["-C", &dir.to_string_lossy(), "rev-parse", "--is-inside-work-tree"])
        .output()
        .with_context(|| format!("git rev-parse in {}", dir.display()))?;
    Ok(ok.status.success()
        && String::from_utf8_lossy(&ok.stdout).trim() == "true")
}

fn is_clean_except_lockfiles(dir: &Path) -> Result<bool> {
    if !is_git_work_tree(dir)? {
        return Ok(false);
    }
    let mut paths = git_changed_paths(dir)?;
    paths.retain(|p| !p.is_empty());
    for p in paths {
        if !is_lockfile_path(&p) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn git_changed_paths(dir: &Path) -> Result<Vec<String>> {
    let mut paths = Vec::new();

    let diff = Command::new("git")
        .args(["-C", &dir.to_string_lossy(), "diff", "--name-only", "HEAD"])
        .output()
        .with_context(|| format!("git diff in {}", dir.display()))?;
    if diff.status.success() {
        paths.extend(
            String::from_utf8_lossy(&diff.stdout)
                .lines()
                .map(str::to_string),
        );
    }

    let untracked = Command::new("git")
        .args([
            "-C",
            &dir.to_string_lossy(),
            "ls-files",
            "--others",
            "--exclude-standard",
        ])
        .output()
        .with_context(|| format!("git ls-files in {}", dir.display()))?;
    if untracked.status.success() {
        paths.extend(
            String::from_utf8_lossy(&untracked.stdout)
                .lines()
                .map(str::to_string),
        );
    }

    Ok(paths)
}

/// True when `path` is an allowlisted lockfile (any directory component).
pub fn is_lockfile_path(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    LOCKFILE_BASENAMES.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command as Proc;

    fn init_git(dir: &Path) {
        Proc::new("git")
            .args(["init", "-q"])
            .current_dir(dir)
            .status()
            .expect("git init");
        Proc::new("git")
            .args(["config", "user.email", "t@example.com"])
            .current_dir(dir)
            .status()
            .expect("git config email");
        Proc::new("git")
            .args(["config", "user.name", "test"])
            .current_dir(dir)
            .status()
            .expect("git config name");
    }

    #[test]
    fn max_depth_includes_shallow_and_deep() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("a/b")).unwrap();
        init_git(&root.join("a"));
        init_git(&root.join("a/b"));

        let listed = list_directories(root, 2, Filters { git: true, ..Default::default() }).unwrap();
        let names: Vec<_> = listed.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        assert!(names.contains(&"a/".to_string()));
        assert!(names.contains(&"a/b/".to_string()));
    }

    #[test]
    fn max_depth_one_excludes_deep() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("a/b")).unwrap();
        init_git(&root.join("a"));
        init_git(&root.join("a/b"));

        let listed = list_directories(root, 1, Filters { git: true, ..Default::default() }).unwrap();
        let names: Vec<_> = listed.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        assert!(names.contains(&"a/".to_string()));
        assert!(!names.contains(&"a/b/".to_string()));
    }

    #[test]
    fn skips_non_git_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir(root.join("plain")).unwrap();
        fs::create_dir(root.join("repo")).unwrap();
        init_git(&root.join("repo"));

        let listed = list_directories(root, 1, Filters { git: true, ..Default::default() }).unwrap();
        let names: Vec<_> = listed.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        assert!(!names.iter().any(|n| n.starts_with("plain")));
        assert!(names.contains(&"repo/".to_string()));
    }

    #[test]
    fn cargo_filter_requires_cargo_toml() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        fs::create_dir(root.join("with")).unwrap();
        fs::write(root.join("with/Cargo.toml"), "[package]\nname=\"x\"\nversion=\"0.1.0\"\n")
            .unwrap();
        init_git(&root.join("with"));
        fs::create_dir(root.join("without")).unwrap();
        init_git(&root.join("without"));

        let listed = list_directories(
            root,
            1,
            Filters {
                git: true,
                cargo: true,
                ..Default::default()
            },
        )
        .unwrap();
        let names: Vec<_> = listed.iter().map(|p| p.to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["with/"]);
    }

    #[test]
    fn is_lockfile_path_matches_basenames() {
        assert!(is_lockfile_path("Cargo.lock"));
        assert!(is_lockfile_path("crates/foo/Cargo.lock"));
        assert!(!is_lockfile_path("src/main.rs"));
    }
}
