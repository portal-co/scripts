// Package keyguard provides helpers for reading the AI submission key from
// a repository's key.agents_.md (or AGENTS.md fallback), resolving the
// correct anchor commit for a CI context, and scanning submission files for
// the key.
package keyguard

import (
	"bufio"
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"regexp"
	"strings"
)

// keyPattern matches the "Key: AIKEY-<token>" line written by inject_key.
var keyPattern = regexp.MustCompile(`(?m)^Key:\s+(AIKEY-[A-Z2-7a-z2-7]+)\s*$`)

// candidateFiles is the ordered list of files checked for a key, in
// preference order.  inject_key always writes to the first; AGENTS.md is a
// read-only fallback for repos that placed the key there manually.
var candidateFiles = []string{"key.agents_.md", "AGENTS.md"}

// ReadKey reads the AI submission key from the working-tree copy of
// key.agents_.md (or AGENTS.md) inside repoRoot.
// Returns ("", nil) when neither file exists or neither contains a key.
func ReadKey(repoRoot string) (string, error) {
	for _, name := range candidateFiles {
		data, err := os.ReadFile(fmt.Sprintf("%s/%s", repoRoot, name))
		if os.IsNotExist(err) {
			continue
		}
		if err != nil {
			return "", fmt.Errorf("keyguard: read %s: %w", name, err)
		}
		if k := extractKey(data); k != "" {
			return k, nil
		}
	}
	return "", nil
}

// ReadKeyAtCommit reads the AI submission key from a specific git commit
// object.  It tries key.agents_.md then AGENTS.md inside the commit tree.
// Returns ("", nil) when the commit contains neither file or neither has a key.
func ReadKeyAtCommit(repoRoot, commitSHA string) (string, error) {
	for _, name := range candidateFiles {
		data, err := gitShow(repoRoot, commitSHA, name)
		if err != nil {
			// git show exits non-zero when the path doesn't exist in the tree.
			continue
		}
		if k := extractKey(data); k != "" {
			return k, nil
		}
	}
	return "", nil
}

// BaseCommit resolves the anchor commit that the CI check should use as the
// baseline for both reading the expected key and determining which files were
// changed in this submission.
//
// Resolution rules:
//
//   - pull_request event: git merge-base HEAD origin/$GITHUB_BASE_REF
//   - push event with a parent commit: HEAD^ (the immediate parent)
//   - push of an orphan / initial commit: returns ("", nil) — caller should skip
func BaseCommit(repoRoot string) (string, error) {
	event := strings.TrimSpace(os.Getenv("GITHUB_EVENT_NAME"))
	baseRef := strings.TrimSpace(os.Getenv("GITHUB_BASE_REF"))

	if event == "pull_request" && baseRef != "" {
		// Fetch the base ref so merge-base works even with a shallow clone.
		_ = runGit(repoRoot, "fetch", "--no-tags", "origin", baseRef)
		sha, err := gitOutput(repoRoot, "merge-base", "HEAD", "origin/"+baseRef)
		if err != nil {
			return "", fmt.Errorf("keyguard: git merge-base HEAD origin/%s: %w", baseRef, err)
		}
		return sha, nil
	}

	// push (or local): use the immediate parent commit.
	sha, err := gitOutput(repoRoot, "rev-parse", "HEAD^")
	if err != nil {
		// HEAD^ fails on an orphan / initial commit — no anchor, skip.
		return "", nil
	}
	return sha, nil
}

// ChangedFiles returns the list of files changed between anchorSHA and HEAD,
// i.e. the submission diff.  Only regular (non-deleted, non-binary) files are
// included.
//
// For a pull_request anchor this gives the full PR diff; for a push anchor
// (HEAD^) it gives exactly the files changed in that push.
func ChangedFiles(repoRoot, anchorSHA string) ([]string, error) {
	out, err := gitOutput(repoRoot, "diff", "--name-only", "--diff-filter=ACMR", anchorSHA, "HEAD")
	if err != nil {
		return nil, fmt.Errorf("keyguard: git diff: %w", err)
	}
	return nonEmptyLines(out), nil
}

// ScanForKey checks each file path (relative to repoRoot) for the presence of
// key as a literal string.  It returns the subset of paths that do NOT contain
// the key.
func ScanForKey(repoRoot string, paths []string, key string) (missing []string, err error) {
	for _, rel := range paths {
		data, readErr := os.ReadFile(fmt.Sprintf("%s/%s", repoRoot, rel))
		if readErr != nil {
			if os.IsNotExist(readErr) {
				continue // deleted files won't be in the tree
			}
			return nil, fmt.Errorf("keyguard: read %s: %w", rel, readErr)
		}
		if !bytes.Contains(data, []byte(key)) {
			missing = append(missing, rel)
		}
	}
	return missing, nil
}

// ─── internal helpers ────────────────────────────────────────────────────────

func extractKey(data []byte) string {
	m := keyPattern.FindSubmatch(data)
	if m == nil {
		return ""
	}
	return string(m[1])
}

func gitShow(repoRoot, commitSHA, path string) ([]byte, error) {
	ref := commitSHA + ":" + path
	cmd := exec.Command("git", "show", ref)
	cmd.Dir = repoRoot
	return cmd.Output()
}

func gitOutput(repoRoot string, args ...string) (string, error) {
	cmd := exec.Command("git", args...)
	cmd.Dir = repoRoot
	out, err := cmd.Output()
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(string(out)), nil
}

func runGit(repoRoot string, args ...string) error {
	cmd := exec.Command("git", args...)
	cmd.Dir = repoRoot
	return cmd.Run()
}

func nonEmptyLines(s string) []string {
	scanner := bufio.NewScanner(strings.NewReader(s))
	var lines []string
	for scanner.Scan() {
		if l := strings.TrimSpace(scanner.Text()); l != "" {
			lines = append(lines, l)
		}
	}
	return lines
}
