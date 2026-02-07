package main

import (
	"archive/zip"
	"flag"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"

	"github.com/portal-co/scripts/pkg/repoutils"
)

func main() {
	outputFile := flag.String("o", "agents.zip", "output zip file name")
	flag.Parse()

	repos := flag.Args()
	if len(repos) == 0 {
		fmt.Fprintln(os.Stderr, "Usage: agents_zip [-o output.zip] <org/repo1> <org/repo2> ...")
		fmt.Fprintln(os.Stderr, "Examples:")
		fmt.Fprintln(os.Stderr, "  agents_zip portal-co/scripts portal-co/pixie")
		fmt.Fprintln(os.Stderr, "  agents_zip https://github.com/portal-co/scripts")
		fmt.Fprintln(os.Stderr, "  agents_zip /path/to/local/repo")
		os.Exit(1)
	}

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
		org, repoName, isRemote := repoutils.ParseOrgRepo(repo)
		
		if isRemote || org != "" {
			// Use GitHub API
			if org == "" {
				fmt.Fprintf(os.Stderr, "Warning: skipping invalid repo spec: %s\n", repo)
				continue
			}
			fmt.Printf("Fetching %s/%s via API...\n", org, repoName)
			added, err := addRemoteAgentFiles(zipWriter, org, repoName)
			if err != nil {
				fmt.Fprintf(os.Stderr, "Warning: failed to fetch %s/%s: %v\n", org, repoName, err)
				continue
			}
			count += added
		} else {
			// Local filesystem path
			fmt.Printf("Processing local repo %s...\n", repo)
			added, err := addLocalAgentFiles(zipWriter, repo, filepath.Base(repo))
			if err != nil {
				fmt.Fprintf(os.Stderr, "Warning: error processing %s: %v\n", repo, err)
				continue
			}
			count += added
		}
	}

	fmt.Printf("Created %s with %d file(s)\n", *outputFile, count)
}

func addRemoteAgentFiles(zw *zip.Writer, org, repo string) (int, error) {
	files, err := repoutils.ListRepoContents(org, repo, "")
	if err != nil {
		return 0, err
	}

	count := 0
	for _, file := range files {
		if file.Type == "file" && strings.HasSuffix(file.Name, "agents_.md") {
			content, err := downloadFile(file.DownloadURL)
			if err != nil {
				fmt.Fprintf(os.Stderr, "Warning: failed to download %s: %v\n", file.Path, err)
				continue
			}

			archivePath := filepath.Join(repo, file.Path)
			if err := addBytesToZip(zw, content, archivePath); err != nil {
				fmt.Fprintf(os.Stderr, "Warning: failed to add %s: %v\n", file.Path, err)
				continue
			}
			fmt.Printf("Added: %s\n", archivePath)
			count++
		}
	}

	return count, nil
}

func addLocalAgentFiles(zw *zip.Writer, repoPath, repoName string) (int, error) {
	count := 0
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

			if err := addFileToZip(zw, path, archivePath); err != nil {
				fmt.Fprintf(os.Stderr, "Warning: failed to add %s: %v\n", path, err)
				return nil
			}
			fmt.Printf("Added: %s\n", archivePath)
			count++
		}
		return nil
	})
	return count, err
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

func addBytesToZip(zw *zip.Writer, content []byte, archivePath string) error {
	header := &zip.FileHeader{
		Name:   archivePath,
		Method: zip.Deflate,
	}

	writer, err := zw.CreateHeader(header)
	if err != nil {
		return err
	}

	_, err = writer.Write(content)
	return err
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
