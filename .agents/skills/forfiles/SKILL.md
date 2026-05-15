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

## Options

| Flag | Description |
|------|-------------|
| `-C, --cwd <PATH>` | Path template applied as each child's working directory. Occurrences of the placeholder in `PATH` are substituted per-line before chdir. Replaces the old `sh -c "cd <line>; …"` pattern. |
| `--exclude <LINE>` | Repeatable. Drop stdin lines whose trimmed text equals `LINE` (exact match). |
| `--exclude-from <PATH>` | Read excluded lines from a file (trimmed, nonempty); same matching as `--exclude`. |

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

### Run a command inside each subdirectory

Use `-C` instead of wrapping the command in `sh -c "cd <line>; …"`:

```bash
ls | ./forfiles -C '{}' '{}' git status --short
```

Batch scripts in `tools/git/` discover targets with `listrepos` (max depth + `--git` / `--cargo` filters) and pipe into `forfiles`:

```bash
./listrepos --max-depth 3 --git | ./forfiles -C '^' '^' git status --short
```

The placeholder is substituted in both the cwd template and any matching
arguments, so commands that need the line value as an argument continue to
work too.

## Behavior

- All commands start concurrently (no rate limiting by default)
- stdout/stderr from each process is forwarded to the terminal (interleaved)
- Exit code is 0 only if every command exits 0
- Empty lines in stdin are skipped; lines whose trimmed text matches any `--exclude` value or any nonempty trimmed line from `--exclude-from` are dropped before spawning

## Notes

For commands that need retry logic on transient failures, see `retry-forfiles`.
For cleaning file descriptors before exec, pipe through `cleanse`.
