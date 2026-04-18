---
name: forfiles
description: >
  Read lines from stdin and run a command per line in parallel, substituting a
  placeholder token with the line value. Use when running the same command across
  many inputs concurrently — repos, files, URLs, or any line-delimited list.
  Trigger on: "forfiles", "parallel foreach", "run per line", "parallel map".
---

# forfiles

Reads lines from stdin and executes a command for each line concurrently,
replacing a placeholder token with the line value. All lines are spawned
simultaneously; the tool waits for all to finish and exits non-zero if any
command fails.

## Installation

```bash
cargo build -p forfiles
# or: cargo install --path crates/forfiles
./forfiles <placeholder> <command> [args...]
```

## Arguments

| Arg | Description |
|-----|-------------|
| `placeholder` | Token to replace with each input line (e.g. `{}`) |
| `command` | Command to run |
| `args...` | Arguments for the command; any arg equal to `placeholder` is substituted |

## Usage

### Run a command for every repo in a list

```bash
cat repos.txt | ./forfiles {} git -C {} pull
```

### Clone every repo in an org

```bash
gh repo list portal-co --json name -q '.[].name' | \
  ./forfiles {} git clone "git@github.com:portal-co/{}.git"
```

### Run cargo update in every subdirectory

```bash
ls -d */ | ./forfiles {} cargo update --manifest-path {}/Cargo.toml
```

### Process files in parallel

```bash
find . -name '*.json' | ./forfiles {} jq '.' {}
```

## Behavior

- All commands start concurrently (no rate limiting by default)
- stdout/stderr from each process is forwarded to the terminal (interleaved)
- Exit code is 0 only if every command exits 0
- Empty lines in stdin are skipped

## Notes

For commands that need retry logic on transient failures, see `retry-forfiles`.
For cleaning file descriptors before exec, pipe through `cleanse`.
