// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! Remove `target/` and `node_modules/` directories from a directory tree.
//!
//! Useful before sharing a project over a network mount where paths or
//! architectures differ between machines, which causes stale build artifacts
//! to produce mysterious and hard-to-diagnose failures.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Parser;
use walkdir::WalkDir;

const DIRS_TO_REMOVE: &[&str] = &["target", "node_modules"];

#[derive(Parser, Debug)]
#[command(
    name = "clean-build-dirs",
    about = "Remove target/ and node_modules/ trees for cross-machine share hygiene"
)]
struct Args {
    /// Only print what would be removed, without deleting anything
    #[arg(long)]
    dry_run: bool,

    /// Directory trees to scan. Defaults to the current directory.
    #[arg(value_name = "DIR")]
    roots: Vec<PathBuf>,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("clean-build-dirs: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args = Args::parse();
    let roots = if args.roots.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        args.roots
    };

    let mut removed = 0u32;
    let mut errors = 0u32;

    for root in &roots {
        for path in find_dirs(root) {
            if args.dry_run {
                println!("would remove {}", path.display());
                removed += 1;
            } else {
                match fs::remove_dir_all(&path) {
                    Ok(()) => {
                        println!("removed {}", path.display());
                        removed += 1;
                    }
                    Err(e) => {
                        eprintln!("clean-build-dirs: {}: {e}", path.display());
                        errors += 1;
                    }
                }
            }
        }
    }

    if args.dry_run {
        eprintln!("clean-build-dirs: dry-run: {} would be removed", removed);
    } else {
        eprintln!("clean-build-dirs: {} removed, {} errors", removed, errors);
    }

    Ok(())
}

fn find_dirs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut it = WalkDir::new(root).follow_links(false).into_iter();
    loop {
        let entry = match it.next() {
            None => break,
            Some(Err(_)) => continue,
            Some(Ok(e)) => e,
        };
        if entry.file_type().is_dir() {
            let name = entry.file_name();
            if DIRS_TO_REMOVE
                .iter()
                .any(|&s| OsStr::new(s) == name)
            {
                out.push(entry.into_path());
                it.skip_current_dir();
            }
        }
    }
    out
}
