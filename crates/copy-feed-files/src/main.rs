// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Copy feed-out.md files from org repos to feed-in.md in the current repo.

use std::path::{Path, PathBuf};

use anyhow::anyhow;
use anyhow::{Context, Result};
use clap::Parser;
use env_traits::{FileEnv, GitEnv, GitHubEnv, GitHubFile};

#[derive(Parser, Debug)]
#[command(about = "Copy *.{repo}.feed-out.md files from org repos to feed-in.md")]
pub struct Opts {
    /// Repository name to filter for (defaults to current repo name).
    #[arg(long)]
    pub repo: Option<String>,
}

pub fn run<F: FileEnv, H: GitHubEnv, G: GitEnv>(
    file: &F,
    gh: &H,
    git: &G,
    opts: &Opts,
) -> Result<()>
where
    F::Error: Send + Sync + 'static,
    G::Error: Send + Sync + 'static,
    H::Error: Send + Sync + 'static,
{
    let repo_name = match &opts.repo {
        Some(name) => name.clone(),
        None => {
            let root = git.repo_root()?;
            Path::new(&root)
                .file_name()
                .and_then(|n| n.to_str())
                .map(String::from)
                .context("could not determine repo name")?
        }
    };

    let org = match gh.current_owner() {
        Ok(o) => o,
        Err(e) => return Err(anyhow!("failed to get organization: {:?}", e)),
    };

    let repo_root = git.repo_root().context("failed to get repo root")?;

    println!(
        "Searching for *{}.feed-out.md files in {} organization...",
        repo_name, org
    );

    let repos = match gh.list_repos(&org, 1000) {
        Ok(r) => r,
        Err(e) => return Err(anyhow!("failed to list org repos: {:?}", e)),
    };

    let feed_suffix = format!(".{}.feed-out.md", repo_name);
    let mut copied_count = 0;

    for repo in repos.iter() {
        let files = match search_files_in_repo(gh, &org, &repo, &feed_suffix) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Warning: error searching {}: {:?}", repo, e);
                continue;
            }
        };

        for file_info in files {
            if let Err(e) = copy_feed_file(file, gh, &file_info, &repo_root, &repo_name) {
                eprintln!("Error copying {}: {:?}", file_info.path, e);
            } else {
                println!("Copied {}/{}:{}", org, repo, file_info.path);
                copied_count += 1;
            }
        }
    }

    println!("\nCopied {} feed file(s)", copied_count);
    Ok(())
}

fn search_files_in_repo<H: GitHubEnv>(
    gh: &H,
    org: &str,
    repo: &str,
    suffix: &str,
) -> Result<Vec<GitHubFile>>
where
    H::Error: Send + Sync + 'static,
{
    let all_files = gh
        .list_contents(org, repo, "")
        .context("failed to list contents")?;
    let matching: Vec<GitHubFile> = all_files
        .into_iter()
        .filter(|f| f.kind == "file" && f.name.ends_with(suffix))
        .map(|f| GitHubFile {
            name: f.name,
            path: f.path,
            kind: f.kind,
            download_url: f.download_url,
        })
        .collect();
    Ok(matching)
}

fn copy_feed_file<F: FileEnv, H: GitHubEnv>(
    file: &F,
    gh: &H,
    file_info: &GitHubFile,
    repo_root: &str,
    repo_name: &str,
) -> Result<()>
where
    F::Error: Send + Sync + 'static,
{
    let download_url = file_info
        .download_url
        .as_ref()
        .context("file has no download_url")?;

    let content = match gh.download_file(download_url) {
        Ok(c) => c,
        Err(e) => return Err(anyhow!("failed to download file: {:?}", e)),
    };

    let feed_out_suffix = format!(".{}.feed-out.md", repo_name);
    if !file_info.path.ends_with(&feed_out_suffix) {
        anyhow::bail!("file doesn't have expected suffix");
    }

    let base_name = file_info.path.trim_end_matches(&feed_out_suffix);
    let new_path = PathBuf::from(repo_root).join(format!("{}.feed-in.md", base_name));

    if let Some(parent) = new_path.parent() {
        file.create_dir_all(&parent.to_string_lossy())
            .with_context(|| format!("failed to create dir {}", parent.display()))?;
    }

    file.write_file(&new_path.to_string_lossy(), &content)
        .with_context(|| format!("failed to write {}", new_path.display()))?;

    Ok(())
}

// downloads are performed via GitHubEnv::download_file

fn main() {
    use env_real::{GhCliGitHubEnv, OsFileEnv, ProcessGitEnv};
    let opts = Opts::parse();
    if let Err(e) = run(&OsFileEnv, &GhCliGitHubEnv, &ProcessGitEnv, &opts) {
        eprintln!("copy-feed-files: {e}");
        std::process::exit(1);
    }
}
