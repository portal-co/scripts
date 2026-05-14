# Tools & Skills

This directory contains a collection of utility scripts and programs designed for automation and bulk operations. These tools are intended for use by both human developers and AI agents (as "skills").

## Usage for AI Agents

When acting as an agent, you can use these tools to perform complex multi-repo operations or parallel tasks. Prefer these scripts over manual loops when appropriate.

- **Base Path**: `/Users/grahamkelly/portal-hot/scripts/tools`
- **Execution**: Most scripts are shell scripts (`.sh`) or Go programs (`go run ...`).

## Tool Catalog

### Core Utilities

#### [forfiles](./forfiles)
A workspace binary (Rust) that reads lines from `stdin` and executes a command for each line in parallel.
- **Usage**: `ls | ./tools/forfiles [OPTIONS] '^' <command> [args...]`
- **Placeholder**: `^` is replaced by the input line in both `command`/`args` and the `--cwd` template.
- **Options**:
  - `-C, --cwd <PATH>` &nbsp;Path template applied as each child's working directory; occurrences of the placeholder in `PATH` are substituted per-line. Replaces the old `sh -c "cd ^; …"` wrapper used throughout `tools/git/`.
  - `--exclude <LINE>` &nbsp;Repeatable; omit stdin lines whose trimmed text equals `LINE` (exact match).
  - `--exclude-from <PATH>` &nbsp;Same, reading one excluded line per nonempty trimmed file line.

A retrying companion lives at [`./retry-forfiles`](./retry-forfiles); it accepts the same `-C / --cwd` and exclude flags plus `--attempts` and `--delay`.

### Git & Repository Management

These scripts are designed to work on a directory containing multiple git repositories. Scripts generated from `tools/git/gen.py` that run git in each subdirectory (`pullall*`, `pushall*`, `commitall*`, `commitandpushall*`) **skip directories that are not git work trees** via a preliminary `forfiles` pass and `--exclude-from`.

| Script | Description |
|:-------|:------------|
| `addignores.sh` | Adds `target`, `node_modules`, and `.DS_Store` to `.gitignore` in all subdirectories, then sorts them. |
| `codeall.sh` | Opens every subdirectory in a new VS Code window. |
| `commitall.sh` | Stages all changes and commits with message `update` in each git subdirectory (skips non-git dirs). |
| `commitandpushall.sh` | Stages all changes, commits with "update", and pushes in each git subdirectory (skips non-git dirs). |
| `pullall.sh` | Runs `git pull --no-rebase` in each git subdirectory (skips non-git dirs). |
| `pushall.sh` | Runs `git push` in each git subdirectory (skips non-git dirs). |
| `sortallignores.sh` | Deduplicates and sorts the `.gitignore` file in every subdirectory. |
| `fetch-repos.sh` | Fetches and clones/updates all public repositories from a GitHub organization. |
| `fetch-repos-gh.sh` | Similar to `fetch-repos.sh` but uses the `gh` CLI for listing repositories. |

### Language Specific

| Script | Description |
|:-------|:------------|
| `fmtallcargo.sh` | Runs `cargo fmt` and commits in each subdirectory; skips directories that are not git work trees and repos with non-lockfile dirty trees (same lockfile list as `updateallcargo.sh`). |
| `updateallcargo.sh` | Runs `cargo update` and commits in each subdirectory; skips directories that are not git work trees, and repos whose working tree has changes other than common lockfiles (`Cargo.lock`, `package-lock.json`, `yarn.lock`, `pnpm-lock.yaml`). |
| `upgradeallcargo.sh` | Like `updateallcargo.sh` but runs `cargo upgrade` (from [cargo-edit](https://github.com/killercup/cargo-edit)); same skip rules. |
| `updateallcargoplus.sh`| Retries `updateallcargo.sh` and `pushall.sh` in a loop (useful for resolving dependency chains). |

### Misc

| Script | Description |
|:-------|:------------|
| `ocrall.sh` | Runs `ocrmypdf` on all PDF files in the current directory to add a text layer. |

## Development

If adding a new tool:
1. Ensure it is generally useful and contains no hardcoded personal data (IPs, specific paths).
2. Ensure it handles errors gracefully, especially when running in parallel.
3. Update this README.
