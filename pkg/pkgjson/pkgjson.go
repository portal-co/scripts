// Package pkgjson provides helpers for reading and surgically rewriting
// package.json files while preserving their existing formatting and key order.
//
// We avoid round-tripping through encoding/json for the whole document because
// Go's JSON encoder does not preserve key order, which would produce noisy diffs
// on every run.  Instead, individual field values are spliced in-place using a
// targeted approach: parse the document once to learn field positions, then
// build the output by replacing only the bytes that changed.
package pkgjson

import (
	"encoding/json"
	"fmt"
	"os"
	"regexp"
	"strings"
)

// Package is a minimal representation of the fields we care about in a
// package.json.  All other fields are preserved verbatim.
type Package struct {
	Name       string // "name" field
	Version    string // "version" field; "" if absent
	Private    bool   // "private" field
	Workspaces bool   // true if a "workspaces" key is present (any value)
}

// Read parses just the fields we need from a package.json at path.
func Read(path string) (*Package, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("pkgjson: read %s: %w", path, err)
	}
	return parse(data)
}

// parse extracts the fields we care about from raw JSON bytes.
func parse(data []byte) (*Package, error) {
	var raw struct {
		Name       string          `json:"name"`
		Version    string          `json:"version"`
		Private    bool            `json:"private"`
		Workspaces json.RawMessage `json:"workspaces"`
	}
	if err := json.Unmarshal(data, &raw); err != nil {
		return nil, fmt.Errorf("pkgjson: unmarshal: %w", err)
	}
	return &Package{
		Name:       raw.Name,
		Version:    raw.Version,
		Private:    raw.Private,
		Workspaces: len(raw.Workspaces) > 0 && string(raw.Workspaces) != "null",
	}, nil
}

// IsPublishable reports whether a package should be published to npm.
// A package is publishable when it has a non-empty name, is not marked
// private, and is not a bare workspace root (i.e. it has a version field
// or addMissing is true).
func (p *Package) IsPublishable(addMissing bool) bool {
	if p.Name == "" || p.Private {
		return false
	}
	if p.Version == "" && !addMissing {
		return false
	}
	return true
}

// versionRe matches the "version" key and its string value in a JSON document,
// including surrounding whitespace so we can replace the whole value token.
var versionRe = regexp.MustCompile(`("version"\s*:\s*)"([^"]*)"`)

// SetVersion rewrites the "version" field in the package.json at path to
// newVersion.  If the file has no "version" field, one is inserted after the
// "name" field.  The rest of the file is preserved byte-for-byte.
func SetVersion(path, newVersion string) error {
	data, err := os.ReadFile(path)
	if err != nil {
		return fmt.Errorf("pkgjson: read %s: %w", path, err)
	}

	out, err := setVersion(data, newVersion)
	if err != nil {
		return fmt.Errorf("pkgjson: SetVersion %s: %w", path, err)
	}

	return os.WriteFile(path, out, 0644)
}

func setVersion(data []byte, newVersion string) ([]byte, error) {
	if versionRe.Match(data) {
		// Replace existing value in-place.
		result := versionRe.ReplaceAll(data, []byte(`${1}"`+newVersion+`"`))
		return result, nil
	}

	// No version field present — insert after the "name" field value, before
	// the following comma (if any) or closing brace.
	//
	// We match the name value token plus whatever immediately follows it so we
	// can splice cleanly in both pretty-printed and compact JSON.
	//
	// Pattern: "name" : "value"  optionally followed by a comma and/or whitespace.
	nameFieldRe := regexp.MustCompile(`("name"\s*:\s*"[^"]*")(\s*,?)`)
	if !nameFieldRe.Match(data) {
		return nil, fmt.Errorf("neither \"version\" nor \"name\" field found")
	}

	// Detect indentation style from the file (look for the name line's indent).
	indent := detectIndent(data)

	replaced := false
	result := nameFieldRe.ReplaceAllFunc(data, func(match []byte) []byte {
		if replaced {
			return match // only replace the first occurrence
		}
		replaced = true

		sub := nameFieldRe.FindSubmatch(match)
		nameToken := sub[1] // e.g.  "name": "@foo/bar"
		comma := sub[2]     // e.g.  ","  or  ""

		// Determine whether the file uses newlines (pretty) or is compact.
		// A pretty-printed file has at least one field on its own indented line.
		prettyRe := regexp.MustCompile(`(?m)^[ \t]+"`)
		if prettyRe.Match(data) {
			// Pretty-printed: insert a new line with matching indent.
			return []byte(string(nameToken) + ",\n" + indent + `"version": "` + newVersion + `"` + string(comma))
		}
		// Compact: insert inline.
		return []byte(string(nameToken) + `,"version":"` + newVersion + `"` + string(comma))
	})
	return result, nil
}

// detectIndent returns the whitespace string used to indent fields in a
// pretty-printed JSON document (e.g. "  " or "\t").  Falls back to two spaces.
func detectIndent(data []byte) string {
	// Look for a line that starts with whitespace followed by a JSON key.
	indentRe := regexp.MustCompile(`(?m)^([ \t]+)"`)
	m := indentRe.FindSubmatch(data)
	if m != nil {
		return string(m[1])
	}
	return "  "
}

// BumpVersion increments the semver string version according to part
// ("major", "minor", or "patch") and returns the new version string.
func BumpVersion(version, part string) (string, error) {
	if version == "" {
		return "0.1.0", nil
	}

	// Strip leading 'v' if present.
	version = strings.TrimPrefix(version, "v")

	parts := strings.Split(version, ".")
	if len(parts) != 3 {
		return "", fmt.Errorf("version %q is not semver (expected x.y.z)", version)
	}

	var major, minor, patch int
	if _, err := fmt.Sscanf(parts[0], "%d", &major); err != nil {
		return "", fmt.Errorf("bad major in %q: %w", version, err)
	}
	if _, err := fmt.Sscanf(parts[1], "%d", &minor); err != nil {
		return "", fmt.Errorf("bad minor in %q: %w", version, err)
	}
	if _, err := fmt.Sscanf(parts[2], "%d", &patch); err != nil {
		return "", fmt.Errorf("bad patch in %q: %w", version, err)
	}

	switch strings.ToLower(part) {
	case "major":
		major++
		minor = 0
		patch = 0
	case "minor":
		minor++
		patch = 0
	case "patch":
		patch++
	default:
		return "", fmt.Errorf("unknown bump part %q (want major, minor, or patch)", part)
	}

	return fmt.Sprintf("%d.%d.%d", major, minor, patch), nil
}
