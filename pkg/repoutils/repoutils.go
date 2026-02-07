// Package repoutils provides common repository handling utilities for tools
package repoutils

import (
	"encoding/json"
	"fmt"
	"os/exec"
	"path/filepath"
	"strings"
)

// GetCurrentRepoName returns the name of the current git repository
func GetCurrentRepoName() (string, error) {
	root, err := GetRepoRoot()
	if err != nil {
		return "", err
	}
	return filepath.Base(root), nil
}

// GetRepoRoot returns the absolute path to the repository root
func GetRepoRoot() (string, error) {
	cmd := exec.Command("git", "rev-parse", "--show-toplevel")
	output, err := cmd.Output()
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(string(output)), nil
}

// GetOrganization returns the organization/owner of the current repository
func GetOrganization() (string, error) {
	cmd := exec.Command("gh", "repo", "view", "--json", "owner", "--jq", ".owner.login")
	output, err := cmd.Output()
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(string(output)), nil
}

// GetOrgRepos returns a list of all repositories in an organization
func GetOrgRepos(org string, limit int) ([]string, error) {
	if limit <= 0 {
		limit = 1000
	}
	limitStr := fmt.Sprintf("%d", limit)
	
	cmd := exec.Command("gh", "repo", "list", org, "--limit", limitStr, "--json", "name", "--jq", ".[].name")
	output, err := cmd.Output()
	if err != nil {
		return nil, err
	}

	var repos []string
	for _, line := range strings.Split(strings.TrimSpace(string(output)), "\n") {
		if line != "" {
			repos = append(repos, line)
		}
	}
	return repos, nil
}

// IsGitURL checks if a string is a git URL
func IsGitURL(s string) bool {
	return strings.HasPrefix(s, "git@") ||
		strings.HasPrefix(s, "https://") ||
		strings.HasPrefix(s, "http://") ||
		strings.HasPrefix(s, "ssh://") ||
		strings.HasPrefix(s, "git://")
}

// RepoNameFromURL extracts the repository name from a git URL
func RepoNameFromURL(url string) string {
	url = strings.TrimSuffix(url, ".git")
	if idx := strings.LastIndex(url, "/"); idx >= 0 {
		return url[idx+1:]
	}
	if idx := strings.LastIndex(url, ":"); idx >= 0 {
		return url[idx+1:]
	}
	return url
}

// CloneRepo clones a repository to a destination directory
func CloneRepo(url, destDir string) error {
	cmd := exec.Command("git", "clone", "--depth", "1", url, destDir)
	return cmd.Run()
}

// ListRepoContents lists the contents of a repository using GitHub API
func ListRepoContents(org, repo, path string) ([]GitHubFile, error) {
	url := "https://api.github.com/repos/" + org + "/" + repo + "/contents/" + path

	cmd := exec.Command("gh", "api", url, "--paginate")
	output, err := cmd.Output()
	if err != nil {
		return nil, err
	}

	var files []GitHubFile
	if err := json.Unmarshal(output, &files); err != nil {
		return nil, err
	}

	var allFiles []GitHubFile
	for _, file := range files {
		if file.Type == "dir" {
			// Recursively search directories
			subFiles, err := ListRepoContents(org, repo, file.Path)
			if err != nil {
				continue // Skip directories we can't access
			}
			allFiles = append(allFiles, subFiles...)
		} else {
			allFiles = append(allFiles, file)
		}
	}

	return allFiles, nil
}

// GitHubFile represents a file in a GitHub repository
type GitHubFile struct {
	Name        string `json:"name"`
	Path        string `json:"path"`
	Type        string `json:"type"`
	DownloadURL string `json:"download_url"`
}

// RunCmd runs a command in a specific directory
func RunCmd(dir, name string, args ...string) error {
	cmd := exec.Command(name, args...)
	cmd.Dir = dir
	return cmd.Run()
}
