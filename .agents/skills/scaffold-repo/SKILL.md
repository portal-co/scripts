---
name: scaffold-repo
description: >
  Scaffold a new Cargo + npm workspace repository on disk with Cargo.toml,
  package.json, .gitignore, and optional git init + initial commit. Use when
  creating a new portal-co workspace repo, bootstrapping a new Rust + JS project,
  or updating an existing repo with new scaffolding features.
  Trigger on: "scaffold repo", "new repo", "create workspace", "bootstrap repo".
---

# scaffold-repo

Creates a new Cargo + npm workspace repository on disk with the standard
portal-co layout: `Cargo.toml`, `package.json`, `.gitignore`, and optionally
a `git init` with an initial commit.

## Installation

```bash
cargo build -p scaffold-repo
# or: cargo install --path crates/scaffold-repo
./scaffold-repo [OPTIONS]
```

## Options

| Flag | Default | Description |
|------|---------|-------------|
| `--name <name>` | prompted | Repository name and directory to create |
| `--out <dir>` | cwd | Parent directory to create the repo in |
| `--description <str>` | `""` | Description for `[workspace.package]` and `package.json` |
| `--license <str>` | `""` | License string (e.g. `"MIT OR Apache-2.0"`) |
| `--workspace-private` | false | Set `publish = false` in `[workspace.package]` |
| `--git-init` | true | Run `git init` and commit generated files |
| `--update` | false | Update an existing repo with new scaffolding features |

## Usage

### Create a new repo

```bash
./scaffold-repo --name my-new-lib --out ~/Code --description "My library" --license "MIT OR Apache-2.0"
```

Creates `~/Code/my-new-lib/` with:
```
Cargo.toml        # workspace manifest
package.json      # npm workspace manifest
.gitignore        # standard Rust + Node ignores
```

With `--git-init` (default), also runs `git init` and creates an initial commit.

### Update an existing repo

```bash
./scaffold-repo --update --name existing-repo --out ~/Code
```

Adds or updates scaffolding files without overwriting custom content.

### Private workspace (not published to crates.io)

```bash
./scaffold-repo --name internal-tool --workspace-private
```

## Generated structure

```toml
# Cargo.toml
[workspace]
members = []
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
description = "..."
license = "..."
```

```json
// package.json
{
  "name": "my-new-lib",
  "version": "0.1.0",
  "private": false,
  "workspaces": []
}
```
