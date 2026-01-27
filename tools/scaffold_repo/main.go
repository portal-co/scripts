package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

func main() {
	name := flag.String("name", "myrepo", "Repository name / directory to create")
	out := flag.String("out", ".", "Parent directory to create the repo in")
	workspacePrivate := flag.Bool("workspace-private", false, "If set, set [workspace.package].publish = false in root Cargo.toml")
	license := flag.String("license", "", "License string to set in generated Cargo.toml files (e.g. MIT OR Apache-2.0). If empty, license field is omitted")
	desc := flag.String("description", "", "Description to put in workspace.package description")
	privateCrates := flag.String("private-crates", "", "Comma-separated crate names to list under crates/ as private (publish = false)")
	publicCrates := flag.String("public-crates", "", "Comma-separated crate names to list under crates/ as public")
	privatePackages := flag.String("private-packages", "", "Comma-separated package names to list under packages/ as private (publish = false)")
	publicPackages := flag.String("public-packages", "", "Comma-separated package names to list under packages/ as public")
	flag.Parse()

	target := filepath.Join(*out, *name)
	if err := os.MkdirAll(target, 0755); err != nil {
		fmt.Fprintf(os.Stderr, "failed to create target dir: %v\n", err)
		os.Exit(1)
	}

	pc := splitNames(*privateCrates)
	uc := splitNames(*publicCrates)
	pp := splitNames(*privatePackages)
	up := splitNames(*publicPackages)

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
	for _, c := range pc {
		p := filepath.Join(cratesDir, c)
		os.MkdirAll(p, 0755)
		writeFile(filepath.Join(p, "_"), "# placeholder for crate to inherit workspace settings\n")
		members = append(members, filepath.ToSlash(filepath.Join("crates", c)))
	}
	for _, c := range uc {
		p := filepath.Join(cratesDir, c)
		os.MkdirAll(p, 0755)
		writeFile(filepath.Join(p, "_"), "# placeholder for crate to inherit workspace settings\n")
		members = append(members, filepath.ToSlash(filepath.Join("crates", c)))
	}

	for _, p := range pp {
		pdir := filepath.Join(packagesDir, p)
		os.MkdirAll(pdir, 0755)
		writeFile(filepath.Join(pdir, "_"), "# placeholder for package to inherit workspace settings\n")
		members = append(members, filepath.ToSlash(filepath.Join("packages", p)))
	}
	for _, p := range up {
		pdir := filepath.Join(packagesDir, p)
		os.MkdirAll(pdir, 0755)
		writeFile(filepath.Join(pdir, "_"), "# placeholder for package to inherit workspace settings\n")
		members = append(members, filepath.ToSlash(filepath.Join("packages", p)))
	}

	// Write root Cargo.toml
	rootCargo := buildRootCargo(members, *workspacePrivate, *license, *desc)
	writeFile(filepath.Join(target, "Cargo.toml"), rootCargo)

	// Generate package.json mimicking ../pixie/package.json
	pkg := buildPackageJSON(*name, *desc, members)
	writeFile(filepath.Join(target, "package.json"), pkg)

	// README
	readme := fmt.Sprintf("# %s\n\nGenerated repository.\n", *name)
	writeFile(filepath.Join(target, "README.md"), readme)

	fmt.Printf("Scaffolded repository at %s\n", target)
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
