// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
//! CLI for `gen-agent-sandbox`.

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use gen_agent_sandbox::{emit, load_config, EmitOpts};

#[derive(Parser, Debug)]
#[command(name = "gen-agent-sandbox")]
#[command(about = "Generate Pi and Claude Code sandbox extensions from YAML")]
struct Opts {
    /// Policy file (YAML).
    #[arg(long)]
    config: PathBuf,

    /// Output directory (created if missing).
    #[arg(long)]
    out_dir: PathBuf,

    #[arg(long)]
    pi_only: bool,

    #[arg(long)]
    claude_only: bool,

    /// Also write `plugin.json` beside hooks (Claude Code plugin layout).
    #[arg(long)]
    emit_plugin_json: bool,
}

fn main() -> Result<()> {
    let opts = Opts::parse();
    if opts.pi_only && opts.claude_only {
        anyhow::bail!("--pi-only and --claude-only are mutually exclusive");
    }
    let cfg = load_config(&opts.config)?;
    emit(
        &cfg,
        &opts.out_dir,
        &opts.config,
        EmitOpts {
            pi_only: opts.pi_only,
            claude_only: opts.claude_only,
            emit_plugin_json: opts.emit_plugin_json,
        },
    )?;
    Ok(())
}
