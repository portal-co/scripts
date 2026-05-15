// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! List directories up to a max depth, optionally filtered for batch git/cargo scripts.
//!
//! Usage:
//!   listrepos --max-depth <N> [--git] [--cargo] [--clean-lockfiles-only]

use std::io::{self, Write};
use std::process;

use clap::Parser;

use listrepos::{list_directories, Filters};

#[derive(Parser, Debug)]
#[command(
    name = "listrepos",
    about = "List directories up to max depth with optional git/cargo/clean filters"
)]
struct Cli {
    /// Maximum directory depth relative to cwd (includes depths 1..=N).
    #[arg(long = "max-depth")]
    max_depth: u32,

    /// Only directories that are git repo roots (contain `.git` as dir or gitfile).
    #[arg(long)]
    git: bool,

    /// Only directories containing a Cargo.toml file.
    #[arg(long)]
    cargo: bool,

    /// Only git dirs whose changes are exclusively allowlisted lockfiles.
    #[arg(long = "clean-lockfiles-only")]
    clean_lockfiles_only: bool,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("listrepos: {e}");
        process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.max_depth == 0 {
        anyhow::bail!("--max-depth must be >= 1");
    }
    if cli.clean_lockfiles_only && !cli.git {
        anyhow::bail!("--clean-lockfiles-only requires --git");
    }
    if cli.cargo && !cli.git {
        anyhow::bail!("--cargo requires --git");
    }

    let filters = Filters {
        git: cli.git,
        cargo: cli.cargo,
        clean_lockfiles_only: cli.clean_lockfiles_only,
    };

    let cwd = std::env::current_dir()?;
    let dirs = list_directories(&cwd, cli.max_depth, filters)?;

    let mut stdout = io::stdout().lock();
    for path in dirs {
        writeln!(stdout, "{}", path.display())?;
    }
    Ok(())
}
