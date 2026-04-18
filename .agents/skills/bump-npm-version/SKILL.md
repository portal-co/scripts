---
name: bump-npm-version
description: >
  Walk a repository's package.json files, bump semver versions, and print
  name@version lines for CI tagging. Use when preparing an npm publish, bumping
  versions before tagging, or wiring up the npm-publish CI workflow.
  Trigger on: "bump npm", "bump version", "npm publish prep", "package.json version".
---

# bump-npm-version

Walks a repository tree for `package.json` files, bumps their semver patch
version, and prints `name@version\tpath` lines consumed by the `npm-publish.yaml`
CI workflow.

## Installation

```bash
cargo build -p bump-npm-version
# or: cargo install --path crates/bump-npm-version
./bump-npm-version [OPTIONS] [REPO_PATH]
```

## Options

| Flag | Description |
|------|-------------|
| `--repo <path>` | Repository root. Defaults to cwd |
| `--dry-run` | Print what would change without writing |
| `--level <patch\|minor\|major>` | Version bump level. Default: `patch` |

## Usage

### Bump all packages in a repo

```bash
./bump-npm-version --repo /path/to/repo
```

Outputs tab-separated lines:

```
my-package@1.2.4	packages/my-package/package.json
other-lib@0.3.1	packages/other-lib/package.json
```

These lines are consumed directly by the `npm-publish.yaml` workflow to create
git tags and publish to npm.

### Preview changes

```bash
./bump-npm-version --dry-run
```

### Minor or major bump

```bash
./bump-npm-version --level minor
```

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | At least one package was bumped |
| 1 | Error |
| 2 | No publishable packages found (no `package.json` with `"private": false`) |

## CI integration

Used by `actions/npm-publish.yaml`. The workflow:
1. Runs `bump-npm-version` and captures its output
2. Commits the version bumps
3. Creates a git tag per `name@version` line
4. Publishes each package to npm

Gated to `github.repository_owner == 'portal-co'`.
