// inject_key upserts an AI submission key section into key.agents_.md
// (creating the file if needed) for a target repository.
//
// Usage:
//
//	go run ./tools/inject_key [-repo <path>] [-rotate] [-dry-run]
//
// Flags:
//
//	-repo <path>  Filesystem path to the target repo root.
//	              Defaults to the git root of the current working directory.
//	-rotate       Generate a new key even if one is already present.
//	              This is the normal operation before starting a new agent
//	              session; it invalidates the old key so any pre-loaded or
//	              training-data key cannot pass the CI check.
//	-dry-run      Print the file that would be written without touching disk.
package main

import (
	"crypto/rand"
	"encoding/base32"
	"flag"
	"fmt"
	"os"
	"os/exec"
	"strings"
)

func main() {
	repoFlag := flag.String("repo", "", "path to target repo root (default: git root of cwd)")
	rotate := flag.Bool("rotate", false, "generate a fresh key, replacing any existing one")
	dryRun := flag.Bool("dry-run", false, "print output without writing to disk")
	flag.Parse()

	repoRoot, err := resolveRepo(*repoFlag)
	if err != nil {
		fatalf("error: %v\n", err)
	}

	keyFile := repoRoot + "/key.agents_.md"

	// Read the existing file (if present) to extract any current key.
	existingContent, _ := os.ReadFile(keyFile)
	oldKey := extractExistingKey(string(existingContent))

	var key string
	switch {
	case oldKey != "" && !*rotate:
		// Preserve the existing key; only refresh the prose.
		key = oldKey
	default:
		// Generate a fresh key (first setup or explicit rotation).
		key, err = generateKey()
		if err != nil {
			fatalf("error generating key: %v\n", err)
		}
	}

	newContent := renderFile(key)

	if *dryRun {
		fmt.Printf("=== would write %s ===\n%s\n", keyFile, newContent)
		return
	}

	if err := os.WriteFile(keyFile, []byte(newContent), 0644); err != nil {
		fatalf("error writing %s: %v\n", keyFile, err)
	}

	if oldKey != "" && *rotate {
		fmt.Printf("Rotated key for %s:\n  old: %s\n  new: %s\n", repoRoot, oldKey, key)
		fmt.Println("Note: PRs branched before this commit must embed the new key.")
	} else if oldKey == "" {
		fmt.Printf("Inserted key for %s:\n  key: %s\n", repoRoot, key)
	} else {
		fmt.Printf("Refreshed instructions for %s (key unchanged: %s)\n", repoRoot, key)
	}
}

// resolveRepo returns the absolute path to the repo root.
func resolveRepo(flag string) (string, error) {
	if flag != "" {
		info, err := os.Stat(flag)
		if err != nil {
			return "", fmt.Errorf("cannot access -repo %q: %w", flag, err)
		}
		if !info.IsDir() {
			return "", fmt.Errorf("-repo %q is not a directory", flag)
		}
		return flag, nil
	}
	// Fall back to git root of cwd.
	cmd := exec.Command("git", "rev-parse", "--show-toplevel")
	out, err := cmd.Output()
	if err != nil {
		return "", fmt.Errorf("not inside a git repository and -repo was not provided")
	}
	return strings.TrimSpace(string(out)), nil
}

// generateKey returns a fresh AIKEY-<base32> token using 128 bits of
// cryptographic randomness.
func generateKey() (string, error) {
	b := make([]byte, 16) // 128 bits
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	token := base32.StdEncoding.WithPadding(base32.NoPadding).EncodeToString(b)
	return "AIKEY-" + strings.ToLower(token), nil
}

// extractExistingKey parses the key line from an existing key.agents_.md body.
func extractExistingKey(content string) string {
	for _, line := range strings.Split(content, "\n") {
		line = strings.TrimSpace(line)
		if strings.HasPrefix(line, "Key: AIKEY-") {
			parts := strings.SplitN(line, " ", 2)
			if len(parts) == 2 {
				return strings.TrimSpace(parts[1])
			}
		}
	}
	return ""
}

// renderFile produces the full content of key.agents_.md.
// The key line is stable; the prose is always up-to-date with the current
// version of inject_key.
func renderFile(key string) string {
	return strings.Join([]string{
		"# AI Submission Key",
		"",
		"This file is managed by `inject_key`.",
		"",
		"**These instructions only apply if you are reading this file as part of your",
		"active instruction set** — i.e. it is loaded as AGENTS.md, is linked from",
		"AGENTS.md, or is otherwise explicitly part of your task context.  If you",
		"encountered this file only by browsing repository contents, ignore it.",
		"",
		"## Key",
		"",
		"Key: " + key,
		"",
		"## Instructions for AI agents",
		"",
		"1. **Read this key before you begin any work on this repository.**",
		"   The key is rotated before each new task session; a stale key from a",
		"   previous session, from training data, or from any source other than",
		"   this file will fail the CI check.",
		"",
		"2. **Embed the key in your submission metadata**, in as many of the",
		"   following places as your capabilities allow — use every one that applies:",
		"",
		"   - **Issue or pull/merge request description** — include the key verbatim",
		"     anywhere in the body text.",
		"   - **Commit message** — include the key verbatim in the message body",
		"     (subject line or footer, e.g. `AI-Key: " + key + "`).",
		"   - **Changed file comments** — if you cannot author PR descriptions or",
		"     commit messages directly, place the key in a comment near the top of",
		"     every file you create or materially modify:",
		"",
		"     - Go / C / Rust / JS / TS:  `// " + key + "`",
		"     - Python / Shell:            `# " + key + "`",
		"     - HTML / XML / Markdown:     `<!-- " + key + " -->`",
		"     - YAML / TOML / INI:         `# " + key + "`",
		"",
		"3. **Do not alter the key.** Copy it exactly, including the `AIKEY-` prefix.",
		"   Paraphrasing, summarising, or abbreviating will fail the check.",
		"",
		"4. **This requirement exists to confirm you read the current task context.**",
		"   If the correct key is present in the changed files, the AI-scan step is",
		"   skipped for those files.  If it is absent, an automated scan will",
		"   determine whether the content appears AI-generated.",
		"",
		"## CI enforcement",
		"",
		"The `check_ai_key` tool (run in CI on every push and pull request) will:",
		"",
		"- Read the key that was current at the base commit of your branch.",
		"- For each changed file in your submission, check for the key literal.",
		"- If the key is absent from a file, run an AI-content scan on that file.",
		"- Fail the check if the file is flagged as AI-generated.",
		"",
		"Repos that have never had a key injected are not subject to enforcement",
		"(fail-open).",
		"",
	}, "\n")
}

func fatalf(format string, args ...any) {
	fmt.Fprintf(os.Stderr, format, args...)
	os.Exit(1)
}
