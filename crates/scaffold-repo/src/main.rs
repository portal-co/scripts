// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Scaffold a new Cargo + npm workspace repository on disk.

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use env_traits::{FileEnv, GitEnv};

#[derive(Parser, Debug)]
#[command(about = "Scaffold a new Cargo + npm workspace repository")]
pub struct Opts {
    /// Repository name / directory to create.
    #[arg(long)]
    pub name: Option<String>,

    /// Parent directory to create the repo in.
    #[arg(long)]
    pub out: Option<PathBuf>,

    /// Description for workspace.package and package.json.
    #[arg(long, default_value = "")]
    pub description: String,

    /// License string (e.g. "MIT OR Apache-2.0").
    #[arg(long, default_value = "")]
    pub license: String,

    /// Set publish = false in [workspace.package].
    #[arg(long)]
    pub workspace_private: bool,

    /// Initialise a git repo and commit generated files.
    #[arg(long, default_value = "true")]
    pub git_init: bool,

    /// Update an existing repository with new scaffolding features.
    #[arg(long)]
    pub update: bool,
}

// ── Pure builders ─────────────────────────────────────────────────────────────

pub fn build_root_cargo(
    workspace_private: bool,
    license: &str,
    desc: &str,
) -> String {
    let mut lines = vec!["[workspace]".to_string(), "members = []".to_string(), "resolver = \"2\"".to_string()];
    if workspace_private || !license.is_empty() || !desc.is_empty() {
        lines.push(String::new());
        lines.push("[workspace.package]".to_string());
        lines.push("version = \"0.1.0\"".to_string());
        if workspace_private {
            lines.push("publish = false".to_string());
        }
        if !desc.is_empty() {
            lines.push(format!("description = \"{desc}\""));
        }
        if !license.is_empty() {
            lines.push(format!("license = \"{license}\""));
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

pub fn build_package_json(name: &str, desc: &str) -> String {
    let pkg_name = format!("@portal-solutions/{}", name.to_lowercase().replace(' ', "-"));
    let desc = if desc.is_empty() { "Generated workspace" } else { desc };
    // Produce compact JSON matching the Go json.MarshalIndent with "" indent
    let obj = serde_json::json!({
        "name": pkg_name,
        "description": desc,
        "workspaces": [],
        "type": "module",
        "devDependencies": { "zshy": "^0.7.0" }
    });
    serde_json::to_string(&obj).unwrap()
}

pub fn build_gitignore() -> &'static str {
    "# Rust\ntarget/\nCargo.lock\n**/*.rs.bk\n*.pdb\n\n\
     # Node.js\nnode_modules/\nnpm-debug.log*\npackage-lock.json\n\n\
     # Build outputs\ndist/\nbuild/\nout/\n\n\
     # IDE\n.vscode/\n.idea/\n*.swp\n*.swo\n*~\n.DS_Store\n\n\
     # Logs\n*.log\nlogs/\n\n\
     # Environment\n.env\n.env.local\n"
}

// ── Core logic ────────────────────────────────────────────────────────────────

pub fn scaffold<F: FileEnv, G: GitEnv>(file: &F, git: &G, opts: &Opts) -> Result<()>
where
    F::Error: Send + Sync + 'static,
    G::Error: Send + Sync + 'static,
{
    let name = opts.name.as_deref().ok_or_else(|| anyhow::anyhow!("--name is required"))?;
    let out  = opts.out.as_deref().ok_or_else(|| anyhow::anyhow!("--out is required"))?;
    if opts.description.is_empty() {
        anyhow::bail!("--description is required");
    }

    let target = out.join(name);
    file.create_dir_all(&target.to_string_lossy())?;
    file.create_dir_all(&target.join("crates").to_string_lossy())?;
    file.create_dir_all(&target.join("packages").to_string_lossy())?;

    file.write_file(
        &target.join("crates/_").to_string_lossy(),
        b"# marker to allow git track empty crates dir\n",
    )?;
    file.write_file(
        &target.join("packages/_").to_string_lossy(),
        b"# marker to allow git track empty packages dir\n",
    )?;

    file.write_file(
        &target.join("Cargo.toml").to_string_lossy(),
        build_root_cargo(opts.workspace_private, &opts.license, &opts.description).as_bytes(),
    )?;
    file.write_file(
        &target.join("package.json").to_string_lossy(),
        build_package_json(name, &opts.description).as_bytes(),
    )?;
    file.write_file(
        &target.join("README.md").to_string_lossy(),
        format!("# {name}\n\nGenerated repository.\n").as_bytes(),
    )?;
    file.write_file(&target.join(".gitignore").to_string_lossy(), build_gitignore().as_bytes())?;

    if opts.git_init {
        git.init(&target.to_string_lossy())?;
        git.add_and_commit(&target.to_string_lossy(), "Initial scaffold")?;
    }

    println!("Scaffolded repository at {}", target.display());
    Ok(())
}

pub fn update<F: FileEnv, G: GitEnv>(file: &F, git: &G) -> Result<()>
where
    F::Error: Send + Sync + 'static,
    G::Error: Send + Sync + 'static,
{
    let repo_root = PathBuf::from(git.repo_root()?);
    println!("Updating repository at {}", repo_root.display());

    let crates_dir   = repo_root.join("crates");
    let packages_dir = repo_root.join("packages");

    if !file.dir_exists(&crates_dir.to_string_lossy()) {
        println!("Creating crates/ directory…");
        file.create_dir_all(&crates_dir.to_string_lossy())?;
        file.write_file(&crates_dir.join("_").to_string_lossy(), b"# marker\n")?;
    }
    if !file.dir_exists(&packages_dir.to_string_lossy()) {
        println!("Creating packages/ directory…");
        file.create_dir_all(&packages_dir.to_string_lossy())?;
        file.write_file(&packages_dir.join("_").to_string_lossy(), b"# marker\n")?;
    }

    let gitignore = repo_root.join(".gitignore");
    if !file.file_exists(&gitignore.to_string_lossy()) {
        println!("Creating .gitignore…");
        file.write_file(&gitignore.to_string_lossy(), build_gitignore().as_bytes())?;
    } else {
        println!(".gitignore already exists, skipping…");
    }

    println!("\nUpdate complete! Review changes with 'git status'");
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use env_fake::{FakeFileEnv, FakeGitEnv};

    #[test]
    fn scaffold_creates_expected_files() {
        let file = FakeFileEnv::default();
        let git  = FakeGitEnv::default().with_repo_root("/out/myrepo");
        let opts = Opts {
            name:              Some("myrepo".into()),
            out:               Some(PathBuf::from("/out")),
            description:       "A test repo".into(),
            license:           "MIT".into(),
            workspace_private: false,
            git_init:          false,
            update:            false,
        };
        scaffold(&file, &git, &opts).unwrap();
        assert!(file.file_exists("/out/myrepo/Cargo.toml"));
        assert!(file.file_exists("/out/myrepo/package.json"));
        assert!(file.file_exists("/out/myrepo/.gitignore"));
    }

    #[test]
    fn update_creates_missing_dirs() {
        let file = FakeFileEnv::default();
        let git  = FakeGitEnv::default().with_repo_root("/repo");
        update(&file, &git).unwrap();
        assert!(file.file_exists("/repo/crates/_"));
        assert!(file.file_exists("/repo/packages/_"));
        assert!(file.file_exists("/repo/.gitignore"));
    }

    #[test]
    fn build_root_cargo_contains_license() {
        let cargo = build_root_cargo(false, "MIT OR Apache-2.0", "desc");
        assert!(cargo.contains("MIT OR Apache-2.0"));
    }

    #[test]
    fn build_package_json_contains_name() {
        let json = build_package_json("my-repo", "desc");
        assert!(json.contains("@portal-solutions/my-repo"));
    }
}

fn main() {
    use env_real::{OsFileEnv, ProcessGitEnv};
    let opts = Opts::parse();
    let result = if opts.update {
        update(&OsFileEnv, &ProcessGitEnv)
    } else {
        scaffold(&OsFileEnv, &ProcessGitEnv, &opts)
    };
    if let Err(e) = result {
        eprintln!("scaffold-repo: {e}");
        std::process::exit(1);
    }
}
