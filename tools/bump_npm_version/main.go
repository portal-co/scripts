// bump_npm_version walks a repository's package.json files, bumps the version
// field in every publishable package, and prints a summary to stdout so the
// calling workflow can create per-package git tags.
//
// Usage:
//
//	go run ./tools/bump_npm_version [-repo <path>] [-bump patch|minor|major]
//	                                [-add-missing] [-dry-run]
//
// Flags:
//
//	-repo <path>    Target repo root. Defaults to git root of cwd.
//	-bump <part>    Semver component to increment (default: patch).
//	-add-missing    If a publishable package.json has no "version" field,
//	                add one at "0.1.0" rather than skipping the file.
//	-dry-run        Print what would change without writing any files.
//
// Output (one line per bumped package, to stdout):
//
//	<npm-name>@<new-version>  <relative-path-to-package.json>
//
// The workflow reads this to create git tags and knows which directories to
// run "npm publish" in.
//
// Exit codes:
//
//	0  — at least one package was bumped (or would be in dry-run)
//	2  — no publishable packages found (nothing to do)
//	1  — error
package main

import (
	"flag"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"github.com/portal-co/scripts/pkg/pkgjson"
)

func main() {
	repoFlag := flag.String("repo", "", "path to target repo root (default: git root of cwd)")
	bump := flag.String("bump", "patch", "semver component to bump: major, minor, or patch")
	addMissing := flag.Bool("add-missing", false, "add version 0.1.0 to packages that have no version field")
	dryRun := flag.Bool("dry-run", false, "print changes without writing files")
	flag.Parse()

	repoRoot, err := resolveRepo(*repoFlag)
	if err != nil {
		fatalf("error: %v\n", err)
	}

	files, err := findPackageJSONs(repoRoot)
	if err != nil {
		fatalf("error finding package.json files: %v\n", err)
	}

	bumped := 0
	for _, absPath := range files {
		pkg, err := pkgjson.Read(absPath)
		if err != nil {
			warnf("skipping %s: %v\n", absPath, err)
			continue
		}

		if !pkg.IsPublishable(*addMissing) {
			continue
		}

		oldVersion := pkg.Version
		if oldVersion == "" {
			oldVersion = "" // will be treated as new in BumpVersion
		}

		newVersion, err := pkgjson.BumpVersion(oldVersion, *bump)
		if err != nil {
			warnf("skipping %s: %v\n", absPath, err)
			continue
		}

		rel, _ := filepath.Rel(repoRoot, absPath)

		if *dryRun {
			if oldVersion == "" {
				fmt.Printf("%s@%s  %s  (would add version)\n", pkg.Name, newVersion, rel)
			} else {
				fmt.Printf("%s@%s  %s  (was %s)\n", pkg.Name, newVersion, rel, oldVersion)
			}
			bumped++
			continue
		}

		if err := pkgjson.SetVersion(absPath, newVersion); err != nil {
			warnf("failed to update %s: %v\n", absPath, err)
			continue
		}

		// Machine-readable line: name@version  path
		// Workflow parses this with "while read name path; do ...".
		fmt.Printf("%s@%s\t%s\n", pkg.Name, newVersion, rel)
		bumped++
	}

	if bumped == 0 {
		fmt.Fprintln(os.Stderr, "bump_npm_version: no publishable packages found")
		os.Exit(2)
	}
}

// findPackageJSONs returns all package.json files under repoRoot, excluding
// node_modules and hidden directories.
func findPackageJSONs(repoRoot string) ([]string, error) {
	var results []string

	err := filepath.WalkDir(repoRoot, func(path string, d os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if d.IsDir() {
			name := d.Name()
			// Skip node_modules, hidden dirs, and the Git object store.
			if name == "node_modules" || name == ".git" || (len(name) > 0 && name[0] == '.') {
				return filepath.SkipDir
			}
			return nil
		}
		if d.Name() == "package.json" {
			results = append(results, path)
		}
		return nil
	})

	return results, err
}

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
	cmd := exec.Command("git", "rev-parse", "--show-toplevel")
	out, err := cmd.Output()
	if err != nil {
		return "", fmt.Errorf("not inside a git repository and -repo was not provided")
	}
	return strings.TrimSpace(string(out)), nil
}

func fatalf(format string, args ...any) {
	fmt.Fprintf(os.Stderr, "bump_npm_version: "+format, args...)
	os.Exit(1)
}

func warnf(format string, args ...any) {
	fmt.Fprintf(os.Stderr, "bump_npm_version: warning: "+format, args...)
}
