// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Assemble a zip archive of all `*agents_.md` files from GitHub or local repos.
//!
//! Usage:
//!   agents-zip [-o output.zip] <org/repo | /local/path> ...

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Parser;
use env_traits::{FileEnv, GitHubEnv};
use repoutils::parse_org_repo;

#[derive(Parser, Debug)]
#[command(about = "Build a zip of agents_.md files from GitHub or local repos")]
pub struct Opts {
    /// Output zip file path.
    #[arg(short = 'o', default_value = "agents.zip")]
    pub output: PathBuf,

    /// One or more repo specs: org/repo, GitHub URL, or local path.
    #[arg(required = true)]
    pub repos: Vec<String>,
}

pub fn run<F: FileEnv, H: GitHubEnv>(file: &F, gh: &H, opts: &Opts) -> Result<usize> {
    let mut zip_bytes: Vec<u8> = Vec::new();
    let mut total = 0usize;

    {
        let cursor = std::io::Cursor::new(&mut zip_bytes);
        let mut zw = zip::ZipWriter::new(cursor);
        let zip_opts: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        for repo_spec in &opts.repos {
            let parsed = parse_org_repo(repo_spec);

            if !parsed.org.is_empty() {
                // Remote GitHub repo
                eprintln!("Fetching {}/{} via API…", parsed.org, parsed.repo);
                let files = match gh.list_contents(&parsed.org, &parsed.repo, "") {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("Warning: failed to list {}/{}: {e}", parsed.org, parsed.repo);
                        continue;
                    }
                };
                for f in files {
                    if f.kind == "file" && f.name.ends_with("agents_.md") {
                        let url = match &f.download_url {
                            Some(u) => u.clone(),
                            None => continue,
                        };
                        let content = match gh.download_file(&url) {
                            Ok(c) => c,
                            Err(e) => {
                                eprintln!("Warning: failed to download {}: {e}", f.path);
                                continue;
                            }
                        };
                        let archive_path = format!("{}/{}", parsed.repo, f.path);
                        zw.start_file(&archive_path, zip_opts)?;
                        zw.write_all(&content)?;
                        eprintln!("Added: {archive_path}");
                        total += 1;
                    }
                }
            } else {
                // Local path
                let repo_path = PathBuf::from(repo_spec);
                let repo_name = repo_path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| repo_spec.clone());
                eprintln!("Processing local repo {repo_spec}…");

                for entry in file.walk(&repo_path)? {
                    let (path, is_dir) = entry?;
                    if is_dir {
                        continue;
                    }
                    if path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.ends_with("agents_.md"))
                        .unwrap_or(false)
                    {
                        let content = file.read_file(&path)?;
                        let rel = path
                            .strip_prefix(&repo_path)
                            .unwrap_or(&path)
                            .display()
                            .to_string();
                        let archive_path = format!("{repo_name}/{rel}");
                        zw.start_file(&archive_path, zip_opts)?;
                        zw.write_all(&content)?;
                        eprintln!("Added: {archive_path}");
                        total += 1;
                    }
                }
            }
        }
        zw.finish()?;
    }

    file.write_file(&opts.output, &zip_bytes)?;
    eprintln!("Created {} with {total} file(s)", opts.output.display());
    Ok(total)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use env_fake::{FakeFileEnv, FakeGitHubEnv};
    use env_traits::GitHubFile;

    #[test]
    fn local_repo_adds_agents_file() {
        let file = FakeFileEnv::default()
            .with_file("/myrepo/key.agents_.md", b"# key");
        let gh = FakeGitHubEnv::default();
        let opts = Opts {
            output: PathBuf::from("/out/agents.zip"),
            repos:  vec!["/myrepo".into()],
        };
        let count = run(&file, &gh, &opts).unwrap();
        assert_eq!(count, 1);
        assert!(file.file_exists(Path::new("/out/agents.zip")));
    }

    #[test]
    fn remote_repo_adds_agents_file() {
        let gh = FakeGitHubEnv::default()
            .with_contents(
                "portal-co",
                "scripts",
                "",
                vec![GitHubFile {
                    name: "key.agents_.md".into(),
                    path: "key.agents_.md".into(),
                    kind: "file".into(),
                    download_url: Some("https://raw/key.agents_.md".into()),
                }],
            )
            .with_download("https://raw/key.agents_.md", b"# key".as_ref());
        let file = FakeFileEnv::default();
        let opts = Opts {
            output: PathBuf::from("/out/agents.zip"),
            repos:  vec!["portal-co/scripts".into()],
        };
        let count = run(&file, &gh, &opts).unwrap();
        assert_eq!(count, 1);
    }
}

fn main() {
    use env_real::{GhCliGitHubEnv, OsFileEnv};
    let opts = Opts::parse();
    match run(&OsFileEnv, &GhCliGitHubEnv, &opts) {
        Ok(n) => println!("Created {} with {n} file(s)", opts.output.display()),
        Err(e) => {
            eprintln!("agents-zip: {e}");
            std::process::exit(1);
        }
    }
}
