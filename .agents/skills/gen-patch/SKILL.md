---
name: gen-patch
description: >
  Generate .cargo/config.toml [patch] sections from local repo checkouts so that
  Cargo resolves workspace dependencies to local paths instead of crates.io or git.
  Use when developing multiple interdependent Rust repos locally, wiring up a
  monorepo-style local override setup, or after cloning a cluster of related repos.
  Trigger on: "gen-patch", "cargo patch", "local patch", "cargo config patch", "local crate override".
---

# gen-patch

Scans a directory of local git repo checkouts and generates a `.cargo/config.toml`
file with `[patch]` sections that make Cargo resolve those crates to local paths
instead of crates.io or git remotes.

## Installation

```bash
cargo build -p gen-patch
# or: cargo install --path crates/gen-patch
./gen-patch [OPTIONS] [REPOS_DIR]
```

## Arguments

| Arg | Default | Description |
|-----|---------|-------------|
| `REPOS_DIR` | cwd | Directory whose immediate subdirectories are local repo checkouts |

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--output <path>` | `<REPOS_DIR>/.cargo/config.toml` | Where to write the generated config |

## Usage

### Generate patches for all repos in a directory

Given a layout like:
```
~/Code/portal-hot/
  scripts/
  waffle-/
  pit-core/
  swibb/
```

```bash
./gen-patch ~/Code/portal-hot
# Writes ~/Code/portal-hot/.cargo/config.toml
```

### Write to a custom location

```bash
./gen-patch --output /tmp/local-patches.toml ~/Code/portal-hot
```

## What it generates

For each subdirectory with both a `Cargo.toml` and an `origin` git remote, emits:

```toml
[patch.'https://github.com/portal-co/waffle-.git']
portal-pc-waffle = { path = "waffle-" }
portal-pc-waffle-backend = { path = "waffle-/crates/waffle-backend" }
# ... one entry per workspace member
```

## How path resolution works

Cargo resolves relative paths in `[patch]` entries from `.cargo/config.toml`
relative to the parent of the `.cargo/` directory — i.e. `REPOS_DIR`. So a path
like `waffle-/crates/waffle-backend` always resolves correctly regardless of
which project inside `REPOS_DIR` is being built.

## Typical workflow

```bash
# Clone all repos into one directory
cd ~/Code/portal-hot
gh repo list portal-co --json sshUrl -q '.[].sshUrl' | \
  forfiles {} git clone {}

# Generate patches
./gen-patch .

# Now any project in the directory picks up local overrides automatically
cd scripts && cargo build   # uses local waffle-, pit-core, etc.
```
