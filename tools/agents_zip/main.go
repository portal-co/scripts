package main

import (
	"archive/zip"
	"flag"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
)

func isGitURL(s string) bool {
	return strings.HasPrefix(s, "git@") ||
		strings.HasPrefix(s, "https://") ||
		strings.HasPrefix(s, "http://") ||
		strings.HasPrefix(s, "ssh://") ||
		strings.HasPrefix(s, "git://")
}

func repoNameFromURL(url string) string {
	url = strings.TrimSuffix(url, ".git")
	if idx := strings.LastIndex(url, "/"); idx >= 0 {
		return url[idx+1:]
	}
	if idx := strings.LastIndex(url, ":"); idx >= 0 {
		return url[idx+1:]
	}
	return url
}

func cloneRepo(url, destDir string) error {
	cmd := exec.Command("git", "clone", "--depth", "1", url, destDir)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	return cmd.Run()
}

func main() {
	outputFile := flag.String("o", "agents.zip", "output zip file name")
	flag.Parse()

	repos := flag.Args()
	if len(repos) == 0 {
		fmt.Fprintln(os.Stderr, "Usage: agents_zip [-o output.zip] <repo1> <repo2> ...")
		os.Exit(1)
	}

	tempDir, err := os.MkdirTemp("", "agents_zip_")
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to create temp directory: %v\n", err)
		os.Exit(1)
	}
	defer os.RemoveAll(tempDir)

	zipFile, err := os.Create(*outputFile)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to create zip file: %v\n", err)
		os.Exit(1)
	}
	defer zipFile.Close()

	zipWriter := zip.NewWriter(zipFile)
	defer zipWriter.Close()

	count := 0
	for _, repo := range repos {
		repoPath := repo
		repoName := filepath.Base(repo)

		if isGitURL(repo) {
			repoName = repoNameFromURL(repo)
			repoPath = filepath.Join(tempDir, repoName)
			fmt.Printf("Cloning %s...\n", repo)
			if err := cloneRepo(repo, repoPath); err != nil {
				fmt.Fprintf(os.Stderr, "Warning: failed to clone %s: %v\n", repo, err)
				continue
			}
		}

		err := filepath.Walk(repoPath, func(path string, info os.FileInfo, err error) error {
			if err != nil {
				return err
			}
			if info.IsDir() {
				return nil
			}
			if strings.HasSuffix(info.Name(), "agents_.md") {
				relPath, err := filepath.Rel(repoPath, path)
				if err != nil {
					relPath = path
				}
				archivePath := filepath.Join(repoName, relPath)

				if err := addFileToZip(zipWriter, path, archivePath); err != nil {
					fmt.Fprintf(os.Stderr, "Warning: failed to add %s: %v\n", path, err)
					return nil
				}
				fmt.Printf("Added: %s\n", archivePath)
				count++
			}
			return nil
		})
		if err != nil {
			fmt.Fprintf(os.Stderr, "Warning: error walking %s: %v\n", repo, err)
		}
	}

	fmt.Printf("Created %s with %d file(s)\n", *outputFile, count)
}

func addFileToZip(zw *zip.Writer, filePath, archivePath string) error {
	file, err := os.Open(filePath)
	if err != nil {
		return err
	}
	defer file.Close()

	info, err := file.Stat()
	if err != nil {
		return err
	}

	header, err := zip.FileInfoHeader(info)
	if err != nil {
		return err
	}
	header.Name = archivePath
	header.Method = zip.Deflate

	writer, err := zw.CreateHeader(header)
	if err != nil {
		return err
	}

	_, err = io.Copy(writer, file)
	return err
}
