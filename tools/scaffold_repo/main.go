package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/portal-co/scripts/pkg/repoutils"
)

func main() {
	name := flag.String("name", "", "Repository name / directory to create (required)")
	out := flag.String("out", "", "Parent directory to create the repo in (required)")
	workspacePrivate := flag.Bool("workspace-private", false, "If set, set [workspace.package].publish = false in root Cargo.toml")
	license := flag.String("license", "", "License string to set in generated Cargo.toml files (e.g. MIT OR Apache-2.0). If empty, license field is omitted")
	desc := flag.String("description", "", "Description to put in workspace.package description (required)")
	gitInit := flag.Bool("git-init", true, "Initialize a git repo and commit generated files; set to false to opt out")
	update := flag.Bool("update", false, "Update an existing repository with new scaffolding features")
	flag.Parse()

	if *update {
		if err := updateExistingRepo(); err != nil {
			fmt.Fprintf(os.Stderr, "error updating repo: %v\n", err)
			os.Exit(1)
		}
		return
	}

	// Validate required fields
	if *name == "" {
		fmt.Fprintf(os.Stderr, "error: -name is required\n")
		os.Exit(1)
	}
	if *out == "" {
		fmt.Fprintf(os.Stderr, "error: -out is required\n")
		os.Exit(1)
	}
	if *desc == "" {
		fmt.Fprintf(os.Stderr, "error: -description is required\n")
		os.Exit(1)
	}

	target := filepath.Join(*out, *name)
	if err := os.MkdirAll(target, 0755); err != nil {
		fmt.Fprintf(os.Stderr, "failed to create target dir: %v\n", err)
		os.Exit(1)
	}

	members := []string{}

	// Ensure crates/ and packages/ exist and create marker files so Git tracks them
	cratesDir := filepath.Join(target, "crates")
	packagesDir := filepath.Join(target, "packages")
	os.MkdirAll(cratesDir, 0755)
	os.MkdirAll(packagesDir, 0755)
	// Create a simple marker file named `_` in the top-level crates and packages dirs
	writeFile(filepath.Join(cratesDir, "_"), "# marker to allow git track empty crates dir\n")
	writeFile(filepath.Join(packagesDir, "_"), "# marker to allow git track empty packages dir\n")

	// Create directories for each named crate/package but only add a marker `_` file inside
	// Note: crates/packages will be added after scaffold finishes.

	// Write root Cargo.toml
	rootCargo := buildRootCargo(members, *workspacePrivate, *license, *desc)
	writeFile(filepath.Join(target, "Cargo.toml"), rootCargo)

	// Generate package.json mimicking ../pixie/package.json
	pkg := buildPackageJSON(*name, *desc, members)
	writeFile(filepath.Join(target, "package.json"), pkg)

	// README
	readme := fmt.Sprintf("# %s\n\nGenerated repository.\n", *name)
	writeFile(filepath.Join(target, "README.md"), readme)

	// .gitignore
	gitignore := buildGitignore()
	writeFile(filepath.Join(target, ".gitignore"), gitignore)

	// Optionally initialize git and commit scaffolded files
	if *gitInit {
		if _, err := lookPath("git"); err != nil {
			fmt.Fprintf(os.Stderr, "git not found; skipping git init\n")
		} else {
			if err := repoutils.RunCmd(target, "git", "init"); err != nil {
				fmt.Fprintf(os.Stderr, "git init failed: %v\n", err)
			} else if err := repoutils.RunCmd(target, "git", "add", "."); err != nil {
				fmt.Fprintf(os.Stderr, "git add failed: %v\n", err)
			} else if err := repoutils.RunCmd(target, "git", "commit", "-m", "Initial scaffold"); err != nil {
				fmt.Fprintf(os.Stderr, "git commit failed: %v\n", err)
			}
		}
	}

	fmt.Printf("Scaffolded repository at %s\n", target)
}

func updateExistingRepo() error {
	repoRoot, err := repoutils.GetRepoRoot()
	if err != nil {
		return fmt.Errorf("not in a git repository: %w", err)
	}

	fmt.Printf("Updating repository at %s\n", repoRoot)

	// Check for missing directories and create them
	cratesDir := filepath.Join(repoRoot, "crates")
	packagesDir := filepath.Join(repoRoot, "packages")
	
	if _, err := os.Stat(cratesDir); os.IsNotExist(err) {
		fmt.Println("Creating crates/ directory...")
		os.MkdirAll(cratesDir, 0755)
		writeFile(filepath.Join(cratesDir, "_"), "# marker to allow git track empty crates dir\n")
	}
	
	if _, err := os.Stat(packagesDir); os.IsNotExist(err) {
		fmt.Println("Creating packages/ directory...")
		os.MkdirAll(packagesDir, 0755)
		writeFile(filepath.Join(packagesDir, "_"), "# marker to allow git track empty packages dir\n")
	}

	// Check for .gitignore
	gitignorePath := filepath.Join(repoRoot, ".gitignore")
	if _, err := os.Stat(gitignorePath); os.IsNotExist(err) {
		fmt.Println("Creating .gitignore...")
		writeFile(gitignorePath, buildGitignore())
	} else {
		fmt.Println(".gitignore already exists, skipping...")
	}

	// Check for Cargo.toml workspace structure
	cargoPath := filepath.Join(repoRoot, "Cargo.toml")
	if _, err := os.Stat(cargoPath); os.IsNotExist(err) {
		fmt.Println("No Cargo.toml found. Creating basic workspace Cargo.toml...")
		repoName, _ := repoutils.GetCurrentRepoName()
		rootCargo := buildRootCargo([]string{}, false, "", fmt.Sprintf("Workspace for %s", repoName))
		writeFile(cargoPath, rootCargo)
	} else {
		fmt.Println("Cargo.toml exists, skipping...")
	}

	// Check for package.json
	pkgPath := filepath.Join(repoRoot, "package.json")
	if _, err := os.Stat(pkgPath); os.IsNotExist(err) {
		fmt.Println("No package.json found. Creating basic workspace package.json...")
		repoName, _ := repoutils.GetCurrentRepoName()
		pkg := buildPackageJSON(repoName, fmt.Sprintf("Workspace for %s", repoName), []string{})
		writeFile(pkgPath, pkg)
	} else {
		fmt.Println("package.json exists, skipping...")
	}

	fmt.Println("\nUpdate complete! Review changes with 'git status'")
	return nil
}

func lookPath(cmd string) (string, error) {
	// Simple wrapper for compatibility
	path := "/usr/bin/" + cmd
	if _, err := os.Stat(path); err == nil {
		return path, nil
	}
	return "", fmt.Errorf("command not found: %s", cmd)
}

func splitNames(s string) []string {
	s = strings.TrimSpace(s)
	if s == "" {
		return nil
	}
	parts := strings.Split(s, ",")
	out := []string{}
	for _, p := range parts {
		p = strings.TrimSpace(p)
		if p != "" {
			out = append(out, p)
		}
	}
	return out
}

func buildRootCargo(members []string, workspacePrivate bool, license, desc string) string {
	lines := []string{"[workspace]"}
	if len(members) == 0 {
		lines = append(lines, "members = []")
	} else {
		lines = append(lines, "members = [")
		for _, m := range members {
			lines = append(lines, fmt.Sprintf("  \"%s\",", m))
		}
		lines = append(lines, "]")
	}
	lines = append(lines, "resolver = \"3\"")
	if workspacePrivate || license != "" || desc != "" {
		lines = append(lines, "\n[workspace.package]")
		if workspacePrivate {
			lines = append(lines, "publish = false")
		}
		lines = append(lines, "version = \"0.1.0\"")
		if desc != "" {
			lines = append(lines, fmt.Sprintf("description = \"%s\"", desc))
		}
		if license != "" {
			lines = append(lines, fmt.Sprintf("license = \"%s\"", license))
		}
	}
	return strings.Join(lines, "\n") + "\n"
}

func buildPackageJSON(name, desc string, members []string) string {
	pkgName := fmt.Sprintf("@portal-solutions/%s", name)
	if !strings.HasPrefix(name, "portal-") {
		// sanitize/normalize name to be npm-friendly: lowercase, replace spaces
		pkgName = fmt.Sprintf("@portal-solutions/%s", strings.ToLower(strings.ReplaceAll(name, " ", "-")))
	}
	if desc == "" {
		desc = "Generated workspace"
	}

	ws := []string{}
	for _, m := range members {
		if strings.HasPrefix(m, "crates/") {
			ws = append(ws, "crates/*")
			break
		}
	}
	for _, m := range members {
		if strings.HasPrefix(m, "packages/") {
			ws = append(ws, "packages/*")
			break
		}
	}

	pkgObj := map[string]interface{}{
		"name":        pkgName,
		"description": desc,
		"workspaces":  ws,
		"type":        "module",
		"devDependencies": map[string]string{
			"zshy": "^0.7.0",
		},
	}
	b, err := json.MarshalIndent(pkgObj, "", "")
	if err != nil {
		// fallback to simple string
		panic(err)

	}

	return string(b)
}

func buildGitignore() string {
	return `# Rust
target/
Cargo.lock
**/*.rs.bk
*.pdb

# Node.js
node_modules/
npm-debug.log*
yarn-debug.log*
yarn-error.log*
package-lock.json

# Build outputs
dist/
build/
out/

# IDE
.vscode/
.idea/
*.swp
*.swo
*~
.DS_Store

# Logs
*.log
logs/

# Environment
.env
.env.local
`
}

func writeFile(path, content string) {
	f, err := os.Create(path)
	if err != nil {
		fmt.Fprintf(os.Stderr, "failed to create %s: %v\n", path, err)
		os.Exit(1)
	}
	defer f.Close()
	if _, err := f.WriteString(content); err != nil {
		fmt.Fprintf(os.Stderr, "failed to write %s: %v\n", path, err)
		os.Exit(1)
	}
}
