package main

import (
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"

	"github.com/portal-co/scripts/pkg/repoutils"
)

func main() {
	// Get current repo name
	repoName, err := repoutils.GetCurrentRepoName()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error getting repo name: %v\n", err)
		os.Exit(1)
	}

	// Get organization
	org, err := repoutils.GetOrganization()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error getting organization: %v\n", err)
		os.Exit(1)
	}

	// Get repository root
	repoRoot, err := repoutils.GetRepoRoot()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error getting repo root: %v\n", err)
		os.Exit(1)
	}

	fmt.Printf("Searching for *%s.feed-out.md files in %s organization...\n", repoName, org)

	// Get all repos in the organization
	repos, err := repoutils.GetOrgRepos(org, 1000)
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

func searchFilesInRepo(org, repo, suffix string) ([]repoutils.GitHubFile, error) {
	var allFiles []repoutils.GitHubFile

	// Search recursively through the repository
	files, err := repoutils.ListRepoContents(org, repo, "")
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

func copyFeedFile(file repoutils.GitHubFile, repoRoot, repoName string) error {
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
