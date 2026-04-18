---
name: agents-zip
description: >
  Assemble a zip archive of all *agents_.md files from GitHub repos or local paths.
  Use when preparing agent context bundles, collecting agent instruction files across
  an org, or packaging AGENTS.md/key.agents_.md files for distribution.
  Trigger on: "agents zip", "agents_.md", "collect agent files", "bundle agent context".
---

# agents-zip

Assembles a zip archive of all `*agents_.md` files found in a list of GitHub
repositories or local paths. Used to bundle agent instruction context for
distribution or inspection.

## Installation

```bash
cargo build -p agents-zip
# or: cargo install --path crates/agents-zip
./agents-zip [OPTIONS] <source> [sources...]
```

## Arguments

Sources can be:
- `org/repo` — a GitHub repository (fetched via the GitHub API)
- `/local/path` — a local directory (walked recursively)

## Options

| Flag | Description |
|------|-------------|
| `-o <file>` | Output zip path. Default: `agents.zip` |

## Usage

### Bundle agents_.md files from GitHub repos

```bash
./agents-zip -o context.zip portal-co/scripts portal-co/my-other-repo
```

### Bundle from a local checkout

```bash
./agents-zip -o context.zip /path/to/local/repo
```

### Mix local and remote

```bash
./agents-zip portal-co/scripts /path/to/local/override
```

## What gets included

Every file whose name matches `*agents_.md`:
- `AGENTS.md` — agent instruction files
- `key.agents_.md` — AI submission key files
- Any other `*agents_.md` files found in the repo tree

The archive preserves relative paths from each repo root, prefixed with
`<repo-name>/`.

## Use in workflows

The zip is consumed by agent tooling to load context across multiple repos
without cloning each one. Pair with `inject-key` to ensure the current session
key is present in the bundled `key.agents_.md` before distributing.
