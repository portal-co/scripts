// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use clean_build_dirs_core::find_dirs;

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
