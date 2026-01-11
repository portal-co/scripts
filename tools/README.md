# Tools & Skills

This directory contains a collection of utility scripts and programs designed for automation and bulk operations. These tools are intended for use by both human developers and AI agents (as "skills").

## Usage for AI Agents

When acting as an agent, you can use these tools to perform complex multi-repo operations or parallel tasks. Prefer these scripts over manual loops when appropriate.

- **Base Path**: `/Users/grahamkelly/portal-hot/scripts/tools`
- **Execution**: Most scripts are shell scripts (`.sh`) or Go programs (`go run ...`).

## Tool Catalog

### Core Utilities

#### [forfiles](./forfiles/forfiles.go)
A Go utility that reads lines from `stdin` and executes a command for each line in parallel.
- **Usage**: `ls | go run tools/forfiles/forfiles.go '^' <command> ^ <args>`
- **Placeholder**: `^` is replaced by the input line.

### Git & Repository Management

These scripts are designed to work on a directory containing multiple git repositories.

| Script | Description |
|:-------|:------------|
| `addignores.sh` | Adds `target`, `node_modules`, and `.DS_Store` to `.gitignore` in all subdirectories, then sorts them. |
| `codeall.sh` | Opens every subdirectory in a new VS Code window. |
| `commitandpushall.sh` | Stages all changes, commits with "update", and pushes in every subdirectory. |
| `pullall.sh` | Runs `git pull --no-rebase` in every subdirectory. |
| `pushall.sh` | Runs `git push` in every subdirectory. |
| `sortallignores.sh` | Deduplicates and sorts the `.gitignore` file in every subdirectory. |
| `fetch-repos.sh` | Fetches and clones/updates all public repositories from a GitHub organization. |
| `fetch-repos-gh.sh` | Similar to `fetch-repos.sh` but uses the `gh` CLI for listing repositories. |

### Language Specific

| Script | Description |
|:-------|:------------|
| `updateallcargo.sh` | Runs `cargo update` and commits the result in every subdirectory. |
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
