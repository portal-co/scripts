package main

import (
	"archive/zip"
	"flag"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

func main() {
	outputFile := flag.String("o", "agents.zip", "output zip file name")
	flag.Parse()

	repos := flag.Args()
	if len(repos) == 0 {
		fmt.Fprintln(os.Stderr, "Usage: agents_zip [-o output.zip] <repo1> <repo2> ...")
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
		err := filepath.Walk(repo, func(path string, info os.FileInfo, err error) error {
			if err != nil {
				return err
			}
			if info.IsDir() {
				return nil
			}
			if strings.HasSuffix(info.Name(), "agents_.md") {
				relPath, err := filepath.Rel(repo, path)
				if err != nil {
					relPath = path
				}
				archivePath := filepath.Join(filepath.Base(repo), relPath)

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
