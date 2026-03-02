// check_ai_key is the CI entrypoint for the AI-submission key check.
//
// It resolves the correct anchor commit for the current CI event, reads the
// expected key from that commit, and then for every changed file verifies
// either (a) the key is present or (b) an AI scanner does not flag the file.
//
// Exit codes:
//
//	0  — all checks passed (or no key was set at the anchor commit)
//	1  — one or more files failed the check
//	2  — unrecoverable error (bad environment, git failure, etc.)
package main

import (
	"fmt"
	"os"
	"strings"

	"github.com/portal-co/scripts/pkg/aiscan"
	"github.com/portal-co/scripts/pkg/keyguard"
	"github.com/portal-co/scripts/pkg/repoutils"
)

func main() {
	os.Exit(run())
}

func run() int {
	// ── 1. Resolve repo root ─────────────────────────────────────────────────
	repoRoot, err := repoutils.GetRepoRoot()
	if err != nil {
		errorf("cannot determine repo root: %v\n", err)
		return 2
	}

	// ── 2. Build the Scanner from environment ────────────────────────────────
	scanner, err := aiscan.FromEnv()
	if err != nil {
		errorf("cannot build AI scanner: %v\n", err)
		return 2
	}
	logf("AI scanner: %T\n", scanner)

	// ── 3. Resolve the anchor commit ─────────────────────────────────────────
	anchor, err := keyguard.BaseCommit(repoRoot)
	if err != nil {
		errorf("cannot resolve base commit: %v\n", err)
		return 2
	}
	if anchor == "" {
		logf("No anchor commit (orphan/initial commit); skipping enforcement.\n")
		return 0
	}
	logf("Anchor commit: %s\n", anchor)

	// ── 4. Read the expected key at the anchor commit ────────────────────────
	key, err := keyguard.ReadKeyAtCommit(repoRoot, anchor)
	if err != nil {
		errorf("cannot read key at anchor commit: %v\n", err)
		return 2
	}
	if key == "" {
		logf("No AI submission key found at anchor commit %s; skipping enforcement.\n", anchor)
		return 0
	}
	logf("Expected key: %s\n", key)

	// ── 5. Determine changed files ───────────────────────────────────────────
	files, err := keyguard.ChangedFiles(repoRoot, anchor)
	if err != nil {
		errorf("cannot determine changed files: %v\n", err)
		return 2
	}
	if len(files) == 0 {
		logf("No changed files to check.\n")
		return 0
	}
	logf("Checking %d changed file(s)...\n", len(files))

	// ── 6. Find files missing the key ────────────────────────────────────────
	missing, err := keyguard.ScanForKey(repoRoot, files, key)
	if err != nil {
		errorf("error scanning files for key: %v\n", err)
		return 2
	}

	if len(missing) == 0 {
		logf("All changed files contain the submission key. ✓\n")
		return 0
	}

	logf("%d file(s) do not contain the key; running AI scan...\n", len(missing))

	// ── 7. AI-scan files that lack the key ───────────────────────────────────
	failures := runAIScan(repoRoot, missing, key, scanner)

	if len(failures) == 0 {
		logf("AI scan found no AI-generated content in files missing the key. ✓\n")
		return 0
	}

	// ── 8. Report failures ───────────────────────────────────────────────────
	fmt.Fprintf(os.Stderr, "\n❌  AI key check failed: %d file(s) appear AI-generated and are missing the submission key.\n\n", len(failures))
	fmt.Fprintf(os.Stderr, "Expected key: %s\n\n", key)
	fmt.Fprintf(os.Stderr, "To fix: embed the key (from key.agents_.md) in each flagged file.\n\n")
	fmt.Fprintf(os.Stderr, "Flagged files:\n")
	for _, f := range failures {
		fmt.Fprintf(os.Stderr, "  %s (confidence %.0f%%)\n", f.path, f.confidence*100)
	}
	fmt.Fprintln(os.Stderr)
	return 1
}

type flaggedFile struct {
	path       string
	confidence float64
}

// runAIScan scans each path (relative to repoRoot) with scanner.
// Files that are not flagged as AI-generated are silently passed.
// The key parameter is unused here (key-presence was already checked) but
// is kept in the signature for future use (e.g. checking key variants).
func runAIScan(repoRoot string, paths []string, _ string, scanner aiscan.Scanner) []flaggedFile {
	var failures []flaggedFile

	for _, rel := range paths {
		fullPath := repoRoot + "/" + rel

		content, err := os.ReadFile(fullPath)
		if err != nil {
			if os.IsNotExist(err) {
				continue // deleted file, not a submission
			}
			logf("warning: could not read %s: %v\n", rel, err)
			continue
		}

		if shouldSkip(rel, content) {
			logf("  skip  %s (binary or non-text)\n", rel)
			continue
		}

		likelyAI, confidence, err := scanner.Scan(fullPath, content)
		if err != nil {
			logf("  warn  %s: scanner error: %v\n", rel, err)
			continue
		}

		if likelyAI {
			logf("  FAIL  %s (AI confidence %.0f%%)\n", rel, confidence*100)
			failures = append(failures, flaggedFile{path: rel, confidence: confidence})
		} else {
			logf("  pass  %s (AI confidence %.0f%%)\n", rel, confidence*100)
		}
	}

	return failures
}

// shouldSkip returns true for binary files or files that are too short to
// meaningfully scan.
func shouldSkip(path string, content []byte) bool {
	// Skip very short files.
	if len(content) < 32 {
		return true
	}
	// Skip binary files: look for a NUL byte in the first 512 bytes.
	check := content
	if len(check) > 512 {
		check = check[:512]
	}
	for _, b := range check {
		if b == 0 {
			return true
		}
	}
	// Skip well-known non-text extensions.
	lower := strings.ToLower(path)
	for _, ext := range []string{
		".png", ".jpg", ".jpeg", ".gif", ".webp", ".ico", ".bmp",
		".pdf", ".zip", ".tar", ".gz", ".bz2", ".xz", ".7z",
		".wasm", ".bin", ".exe", ".dll", ".so", ".dylib",
		".mp3", ".mp4", ".wav", ".ogg", ".flac",
		".ttf", ".otf", ".woff", ".woff2",
		".lock", // Cargo.lock, package-lock.json etc. are machine-generated
	} {
		if strings.HasSuffix(lower, ext) {
			return true
		}
	}
	return false
}

func logf(format string, args ...any) {
	fmt.Printf(format, args...)
}

func errorf(format string, args ...any) {
	fmt.Fprintf(os.Stderr, "check_ai_key: "+format, args...)
}
