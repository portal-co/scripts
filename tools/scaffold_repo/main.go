package main

import (
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
	privateCrates := flag.String("private-crates", "", "Comma-separated crate names to create under crates/ as private (publish = false)")
	publicCrates := flag.String("public-crates", "", "Comma-separated crate names to create under crates/ as public")
	privatePackages := flag.String("private-packages", "", "Comma-separated package names to create under packages/ as private (publish = false)")
	publicPackages := flag.String("public-packages", "", "Comma-separated package names to create under packages/ as public")
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

	// Create crates
	if len(pc)+len(uc) > 0 {
		cratesDir := filepath.Join(target, "crates")
		os.MkdirAll(cratesDir, 0755)
		for _, c := range pc {
			p := filepath.Join(cratesDir, c)
			createCrate(p, c, *license, true)
			members = append(members, filepath.ToSlash(filepath.Join("crates", c)))
		}
		for _, c := range uc {
			p := filepath.Join(cratesDir, c)
			createCrate(p, c, *license, false)
			members = append(members, filepath.ToSlash(filepath.Join("crates", c)))
		}
	}

	// Create packages
	if len(pp)+len(up) > 0 {
		pkgsDir := filepath.Join(target, "packages")
		os.MkdirAll(pkgsDir, 0755)
		for _, p := range pp {
			pdir := filepath.Join(pkgsDir, p)
			createCrate(pdir, p, *license, true)
			members = append(members, filepath.ToSlash(filepath.Join("packages", p)))
		}
		for _, p := range up {
			pdir := filepath.Join(pkgsDir, p)
			createCrate(pdir, p, *license, false)
			members = append(members, filepath.ToSlash(filepath.Join("packages", p)))
		}
	}

	// Write root Cargo.toml
	rootCargo := buildRootCargo(members, *workspacePrivate, *license, *desc)
	writeFile(filepath.Join(target, "Cargo.toml"), rootCargo)

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

func createCrate(dir, name, license string, private bool) {
	os.MkdirAll(dir, 0755)
	src := filepath.Join(dir, "src")
	os.MkdirAll(src, 0755)
	lib := "// Auto-generated lib\n\npub fn hello() -> &'static str { \"hello\" }\n"
	writeFile(filepath.Join(src, "lib.rs"), lib)

	cargo := buildCrateCargo(name, license, private)
	writeFile(filepath.Join(dir, "Cargo.toml"), cargo)

	readme := fmt.Sprintf("# %s\n\nAuto-generated crate %s\n", name, name)
	writeFile(filepath.Join(dir, "README.md"), readme)
}

func buildCrateCargo(name, license string, private bool) string {
	lines := []string{"[package]"}
	lines = append(lines, fmt.Sprintf("name = \"%s\"", name))
	lines = append(lines, "version = \"0.1.0\"")
	lines = append(lines, "edition = \"2021\"")
	if license != "" {
		lines = append(lines, fmt.Sprintf("license = \"%s\"", license))
	}
	if private {
		lines = append(lines, "publish = false")
	}
	lines = append(lines, "\n[dependencies]")
	return strings.Join(lines, "\n") + "\n"
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
