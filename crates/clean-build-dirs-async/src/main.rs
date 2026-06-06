// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use clean_build_dirs_core::find_dirs;

#[derive(Parser, Debug)]
#[command(
    name = "clean-build-dirs-async",
    about = "Concurrently remove target/ and node_modules/ trees for cross-machine share hygiene"
)]
struct Args {
    /// Only print what would be removed, without deleting anything
    #[arg(long)]
    dry_run: bool,

    /// Directory trees to scan. Defaults to the current directory.
    #[arg(value_name = "DIR")]
    roots: Vec<PathBuf>,
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("clean-build-dirs-async: {e:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let args = Args::parse();
    let roots = if args.roots.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        args.roots
    };

    let paths: Vec<PathBuf> = roots.iter().flat_map(|r| find_dirs(r)).collect();

    if args.dry_run {
        for path in &paths {
            println!("would remove {}", path.display());
        }
        eprintln!(
            "clean-build-dirs-async: dry-run: {} would be removed",
            paths.len()
        );
        return Ok(());
    }

    let tasks: Vec<_> = paths
        .into_iter()
        .map(|path| {
            tokio::spawn(async move {
                match tokio::fs::remove_dir_all(&path).await {
                    Ok(()) => Ok(path),
                    Err(e) => Err((path, e)),
                }
            })
        })
        .collect();

    let mut removed = 0u32;
    let mut errors = 0u32;

    for task in tasks {
        match task.await? {
            Ok(path) => {
                println!("removed {}", path.display());
                removed += 1;
            }
            Err((path, e)) => {
                eprintln!("clean-build-dirs-async: {}: {e}", path.display());
                errors += 1;
            }
        }
    }

    eprintln!(
        "clean-build-dirs-async: {} removed, {} errors",
        removed, errors
    );

    Ok(())
}
