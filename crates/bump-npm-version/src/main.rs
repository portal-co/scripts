// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Walk a repository's package.json files, bump versions, and print
//! `name@ver\tpath` lines consumed by the npm-publish CI workflow.
//!
//! Exit codes:
//!   0 — at least one package was bumped
//!   2 — no publishable packages found
//!   1 — error

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;
use env_traits::{FileEnv, GitEnv};
use pkgjson::BumpPart;

#[derive(Parser, Debug)]
#[command(about = "Bump npm package versions in a repository")]
pub struct Opts {
    /// Path to the target repo root.  Defaults to the git root of cwd.
    #[arg(long)]
    pub repo: Option<PathBuf>,

    /// Semver component to bump.
    #[arg(long, default_value = "patch")]
    pub bump: String,

    /// Add version 0.1.0 to packages that have no version field.
    #[arg(long)]
    pub add_missing: bool,

    /// Print changes without writing files.
    #[arg(long)]
    pub dry_run: bool,
}

/// Directories to skip when walking for package.json files.
const SKIP_DIRS: &[&str] = &["node_modules", ".git"];

pub fn find_package_jsons<F: FileEnv>(file: &F, repo_root: &Path) -> Result<Vec<PathBuf>>
where
    F::Error: Send + Sync + 'static,
{
    let mut results = Vec::new();
    for entry in file.walk(&repo_root.to_string_lossy())? {
        let (path, is_dir) = entry?;
        let path_buf = PathBuf::from(&path);
        if is_dir {
            if let Some(name) = path_buf.file_name().and_then(|n| n.to_str()) {
                if SKIP_DIRS.contains(&name) || name.starts_with('.') {
                    continue;
                }
            }
            continue;
        }
        if path_buf.file_name().and_then(|n| n.to_str()) == Some("package.json") {
            // Skip files inside skipped directories.
            let in_skip = path_buf.components().any(|c| {
                let s = c.as_os_str().to_str().unwrap_or("");
                SKIP_DIRS.contains(&s) || (s.starts_with('.') && s != ".")
            });
            if !in_skip {
                results.push(path_buf);
            }
        }
    }
    Ok(results)
}

pub fn run<F: FileEnv, G: GitEnv>(file: &F, git: &G, opts: &Opts) -> ExitCode
where
    F::Error: Send + Sync + 'static,
    G::Error: Send + Sync + 'static,
{
    let repo_root = match resolve_repo(file, git, opts.repo.as_deref()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("bump-npm-version: {e}");
            return ExitCode::from(1);
        }
    };

    let bump_part: BumpPart = match opts.bump.parse() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("bump-npm-version: {e}");
            return ExitCode::from(1);
        }
    };

    let pkg_files = match find_package_jsons(file, &repo_root) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("bump-npm-version: error finding package.json files: {e}");
            return ExitCode::from(1);
        }
    };

    let mut bumped = 0usize;
    for abs_path in &pkg_files {
        let pkg = match pkgjson::read(file, abs_path) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("bump-npm-version: skipping {}: {e}", abs_path.display());
                continue;
            }
        };

        if !pkg.is_publishable(opts.add_missing) {
            continue;
        }

        let new_version = match pkgjson::bump_version(&pkg.version, bump_part) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("bump-npm-version: skipping {}: {e}", abs_path.display());
                continue;
            }
        };

        let rel = abs_path
            .strip_prefix(&repo_root)
            .unwrap_or(abs_path)
            .display()
            .to_string();

        if opts.dry_run {
            if pkg.version.is_empty() {
                println!("{}@{}  {}  (would add version)", pkg.name, new_version, rel);
            } else {
                println!("{}@{}  {}  (was {})", pkg.name, new_version, rel, pkg.version);
            }
            bumped += 1;
            continue;
        }

        if let Err(e) = pkgjson::set_version(file, abs_path, &new_version) {
            eprintln!("bump-npm-version: failed to update {}: {e}", abs_path.display());
            continue;
        }

        // Tab-separated: name@version\tpath  (consumed by npm-publish.yaml)
        println!("{}\t{}", format!("{}@{}", pkg.name, new_version), rel);
        bumped += 1;
    }

    if bumped == 0 {
        eprintln!("bump-npm-version: no publishable packages found");
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    }
}

fn resolve_repo<F: FileEnv, G: GitEnv>(
    file: &F,
    git: &G,
    flag: Option<&Path>,
) -> Result<PathBuf>
where
    F::Error: Send + Sync + 'static,
    G::Error: Send + Sync + 'static,
{
    if let Some(p) = flag {
        if !file.dir_exists(&p.to_string_lossy()) {
            anyhow::bail!("--repo {:?} is not a directory", p);
        }
        return Ok(p.to_path_buf());
    }
    Ok(PathBuf::from(git.repo_root()?))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use env_fake::{FakeFileEnv, FakeGitEnv};

    fn opts(dry_run: bool) -> Opts {
        Opts {
            repo: Some(PathBuf::from("/repo")),
            bump: "patch".into(),
            add_missing: false,
            dry_run,
        }
    }

    fn seed_pkg(file: &FakeFileEnv, path: &str, content: &str) -> FakeFileEnv {
        file.clone().with_file(path, content.as_bytes())
    }

    #[test]
    fn bumps_publishable_package() {
        let pkg_content = r#"{"name":"@foo/bar","version":"1.0.0"}"#;
        let file = FakeFileEnv::default().with_file("/repo/package.json", pkg_content.as_bytes());
        let git  = FakeGitEnv::default().with_repo_root("/repo");
        let code = run(&file, &git, &opts(false));
        assert_eq!(code, ExitCode::SUCCESS);
        let data = file.read_file("/repo/package.json").unwrap();
        let updated = pkgjson::read(&file, Path::new("/repo/package.json")).unwrap();
        assert_eq!(updated.version, "1.0.1");
        let _ = data;
    }

    #[test]
    fn skips_private_package() {
        let pkg_content = r#"{"name":"@foo/bar","version":"1.0.0","private":true}"#;
        let file = FakeFileEnv::default().with_file("/repo/package.json", pkg_content.as_bytes());
        let git  = FakeGitEnv::default().with_repo_root("/repo");
        let code = run(&file, &git, &opts(false));
        // No publishable packages → exit 2
        assert_eq!(code, ExitCode::from(2));
    }

    #[test]
    fn dry_run_does_not_write() {
        let original = r#"{"name":"@foo/bar","version":"1.0.0"}"#;
        let file = FakeFileEnv::default().with_file("/repo/package.json", original.as_bytes());
        let git  = FakeGitEnv::default().with_repo_root("/repo");
        run(&file, &git, &opts(true));
        // File should be unchanged.
        let data = file.read_file("/repo/package.json").unwrap();
        assert_eq!(data, original.as_bytes());
    }
}

fn main() {
    use env_real::{OsFileEnv, ProcessGitEnv};
    let opts = Opts::parse();
    std::process::exit(match run(&OsFileEnv, &ProcessGitEnv, &opts) {
        ExitCode::SUCCESS => 0,
        c if c == ExitCode::from(2) => 2,
        _ => 1,
    });
}
