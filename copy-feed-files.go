package main

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
)

type GitHubFile struct {
	Name        string `json:"name"`
	Path        string `json:"path"`
	Type        string `json:"type"`
	DownloadURL string `json:"download_url"`
}

func main() {
	// Get current repo name
	repoName, err := getCurrentRepoName()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error getting repo name: %v\n", err)
		os.Exit(1)
	}

	// Get organization
	org, err := getOrganization()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error getting organization: %v\n", err)
		os.Exit(1)
	}

	// Get repository root
	repoRoot, err := getRepoRoot()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error getting repo root: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("Searching for *%s.feed-out.md files in %s organization...\n", repoName, org)

	// Get all repos in the organization
	repos, err := getOrgRepos(org)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error getting repos: %v\n", err)
		os.Exit(1)
	}

	feedSuffix := fmt.Sprintf(".%s.feed-out.md", repoName)
	copiedCount := 0

	for _, repo := range repos {
		files, err := searchFilesInRepo(org, repo, feedSuffix)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Warning: Error searching %s: %v\n", repo, err)
			continue
		}

		for _, file := range files {
			if err := copyFeedFile(file, repoRoot, repoName); err != nil {
				fmt.Fprintf(os.Stderr, "Error copying %s: %v\n", file.Path, err)
			} else {
				fmt.Printf("Copied %s/%s:%s\n", org, repo, file.Path)
				copiedCount++
			}
		}
	}

	fmt.Printf("\nCopied %d feed file(s)\n", copiedCount)
}

func getCurrentRepoName() (string, error) {
	cmd := exec.Command("git", "rev-parse", "--show-toplevel")
	output, err := cmd.Output()
	if err != nil {
		return "", err
	}
	repoPath := strings.TrimSpace(string(output))
	return filepath.Base(repoPath), nil
}

func getOrganization() (string, error) {
	cmd := exec.Command("gh", "repo", "view", "--json", "owner", "--jq", ".owner.login")
	output, err := cmd.Output()
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(string(output)), nil
}

func getRepoRoot() (string, error) {
	cmd := exec.Command("git", "rev-parse", "--show-toplevel")
	output, err := cmd.Output()
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(string(output)), nil
}

func getOrgRepos(org string) ([]string, error) {
	cmd := exec.Command("gh", "repo", "list", org, "--limit", "1000", "--json", "name", "--jq", ".[].name")
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

func searchFilesInRepo(org, repo, suffix string) ([]GitHubFile, error) {
	var allFiles []GitHubFile

	// Search recursively through the repository
	files, err := listRepoContents(org, repo, "")
	if err != nil {
		return nil, err
	}

	for _, file := range files {
		if file.Type == "file" && strings.HasSuffix(file.Name, suffix) {
			allFiles = append(allFiles, file)
		}
	}

	return allFiles, nil
}

func listRepoContents(org, repo, path string) ([]GitHubFile, error) {
	url := fmt.Sprintf("https://api.github.com/repos/%s/%s/contents/%s", org, repo, path)

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
			subFiles, err := listRepoContents(org, repo, file.Path)
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

func copyFeedFile(file GitHubFile, repoRoot, repoName string) error {
	// Download file content
	content, err := downloadFile(file.DownloadURL)
	if err != nil {
		return err
	}

	// Transform filename: remove .{repoName}.feed-out.md and add .feed-in.md
	feedOutSuffix := fmt.Sprintf(".%s.feed-out.md", repoName)
	if !strings.HasSuffix(file.Path, feedOutSuffix) {
		return fmt.Errorf("file doesn't have expected suffix")
	}

	baseName := strings.TrimSuffix(file.Path, feedOutSuffix)
	newPath := filepath.Join(repoRoot, baseName+".feed-in.md")

	// Create directory if needed
	if err := os.MkdirAll(filepath.Dir(newPath), 0755); err != nil {
		return err
	}

	// Write file
	return os.WriteFile(newPath, content, 0644)
}

func downloadFile(url string) ([]byte, error) {
	resp, err := http.Get(url)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("bad status: %s", resp.Status)
	}

	return io.ReadAll(resp.Body)
}
