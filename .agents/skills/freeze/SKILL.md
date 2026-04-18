---
name: freeze
description: >
  Record SHA-256 checksums of scripts/files into a lockfile, then validate them
  before executing commands. Use when setting up tamper-evident CI steps, protecting
  shell scripts in GitHub Actions workflows, or verifying files haven't changed since
  they were reviewed. Trigger on: "freeze", "checksum", "lockfile hash", "verify before
  run", "tamper-evident", "pin script".
---

# freeze

Records SHA-256 checksums of files into a TOML lockfile and validates them before
executing commands. Designed for GitHub Actions and other script-dependent workflows
where you need tamper-evident guarantees about which scripts are being run.

## Installation

Build from the scripts workspace:

```bash
cargo build -p freeze
# binary: target/debug/freeze
# or install globally: cargo install --path crates/freeze
```

The `freeze` shell shim in the repo root auto-rebuilds via `run`:

```bash
./freeze <subcommand> [args]
```

## Subcommands

### `freeze record [--lockfile <path>] <files...>`

Compute SHA-256 of each file and write entries to the lockfile.
Default lockfile: `freeze.lock`

After writing, prints the lockfile's own hash and the exact `--lockfile-hash` flag
to embed in CI.

```bash
./freeze record deploy.sh setup.sh actions/install.sh
# Outputs:
#   recorded  8bb3c5a4f727...  deploy.sh
#   recorded  6e76856fa851...  setup.sh
#   freeze: wrote 3 entries to 'freeze.lock'
#   freeze: lockfile hash  89b3a5148f13...
#   freeze: embed in CI:   --lockfile-hash 89b3a5148f13...
```

Commit `freeze.lock` alongside the scripts it covers.

### `freeze check [--lockfile <path>] [--lockfile-hash <sha256>]`

Re-compute SHA-256 for every entry in the lockfile and verify they match.
Exits 0 on success, 1 on any mismatch or missing file.

`--lockfile-hash` verifies the lockfile itself before parsing its contents — embed
this value directly in the CI workflow YAML so the chain of trust starts from a
version-controlled, access-controlled file.

```bash
./freeze check --lockfile-hash 89b3a5148f13adca...
```

### `freeze exec [--lockfile <path>] [--lockfile-hash <sha256>] -- <cmd> [args...]`

Validate checksums (and optionally the lockfile hash), then `exec` the command,
replacing the current process. Refuses to run if any check fails.

```bash
./freeze exec --lockfile-hash 89b3a5148f13adca... -- ./deploy.sh --env prod
```

## GitHub Actions pattern

```yaml
# In the protected workflow YAML (hash baked in here, protected by branch rules):
- name: Deploy
  run: ./freeze exec --lockfile-hash 89b3a5148f13adcadfddfa1e09fa45c23aea2855f0c7446df418698db4335b3d -- ./deploy.sh
```

The chain of trust:
1. `freeze.lock` is committed and reviewed alongside the scripts it covers
2. The lockfile hash is baked into the workflow YAML, which is branch-protected
3. At runtime, the lockfile is verified against the baked hash before any file
   checksums are evaluated — a tampered lockfile is caught immediately

## Lockfile format

TOML, human-readable:

```toml
[[files]]
path = "deploy.sh"
sha256 = "e3b0c44298fc1c149afb4c8996fb92427ae41e4649b934ca495991b7852b855"

[[files]]
path = "setup.sh"
sha256 = "ba7816bf8f01cfea414140de5dae2ec73b00361bbef0469fa72a392050c4a72"
```
