---
name: copy-feed-files
description: >
  Scan all repos in the current GitHub org for *.{current-repo}.feed-out.md files
  and copy them locally as *.feed-in.md. Use when syncing feed files across org
  repos, pulling in cross-repo context for an agent session, or setting up the
  feed-file pipeline between repos.
  Trigger on: "copy feed files", "feed-out", "feed-in", "sync feed", "cross-repo feed".
---

# copy-feed-files

Scans every repo in the current GitHub org for files matching
`*.{current-repo-name}.feed-out.md`, then copies them into the current repo as
`*.feed-in.md`. Implements the cross-repo feed pipeline: repos publish
context for their dependents via `feed-out.md` files, and dependents pull them
in as `feed-in.md`.

## Installation

```bash
cargo build -p copy-feed-files
# or: cargo install --path crates/copy-feed-files
./copy-feed-files [OPTIONS]
```

## Options

| Flag | Description |
|------|-------------|
| `--org <org>` | GitHub org to scan. Defaults to the org of the current repo's remote |
| `--repo <name>` | Current repo name to match against. Defaults to the git remote name |
| `--out <dir>` | Directory to write feed-in files into. Defaults to cwd |
| `--dry-run` | Print what would be copied without writing |

## Usage

### Pull in all feed files for the current repo

```bash
./copy-feed-files
```

Scans all repos in the org for `*.my-repo.feed-out.md`, downloads each one,
and writes it locally as `*.feed-in.md`.

### Explicit org and repo name

```bash
./copy-feed-files --org portal-co --repo scripts
```

### Preview without writing

```bash
./copy-feed-files --dry-run
```

## Feed file convention

| File | Location | Purpose |
|------|----------|---------|
| `context.scripts.feed-out.md` | In repo `foo` | Context from `foo` for repo `scripts` |
| `context.feed-in.md` | In repo `scripts` | Received copy, used as agent context |

Agents and workflows read `*.feed-in.md` files for cross-repo context. The
naming convention means a repo only receives files explicitly addressed to it.

## Authentication

Requires `GITHUB_TOKEN` in the environment with `contents:read` scope on the
org's repositories.
