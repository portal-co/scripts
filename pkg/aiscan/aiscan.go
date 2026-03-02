// Package aiscan provides an interface and implementations for detecting
// AI-generated content in source files.
package aiscan

import (
	"fmt"
	"os"
	"strings"
)

// Scanner decides whether a file's content is likely AI-generated.
// Implementations may be stateless heuristics, remote API calls, or
// anything else satisfying the contract.
type Scanner interface {
	// Scan inspects content (the full text of the file at path) and
	// returns whether it appears AI-generated along with a confidence in [0,1].
	// An error means the scan itself failed; a (false, 0, nil) result means
	// the file was scanned and not flagged.
	Scan(path string, content []byte) (likelyAI bool, confidence float64, err error)
}

// NoopScanner always returns (false, 0, nil). It is selected when
// AI_SCAN_BACKEND=none, disabling AI detection while still allowing
// the key-presence check to run.
type NoopScanner struct{}

func (NoopScanner) Scan(_ string, _ []byte) (bool, float64, error) {
	return false, 0, nil
}

// FromEnv assembles a Scanner from environment variables:
//
//	AI_SCAN_BACKEND=none      → NoopScanner
//	AI_SCAN_BACKEND=http      → HTTPScanner (also selected when AI_SCAN_ENDPOINT is set)
//	AI_SCAN_BACKEND=heuristic → HeuristicScanner (default)
//	(unset)                   → HeuristicScanner
//
// The returned Scanner is ready to use; callers pass it down as a plain
// argument — no global state.
func FromEnv() (Scanner, error) {
	backend := strings.ToLower(strings.TrimSpace(os.Getenv("AI_SCAN_BACKEND")))
	endpoint := strings.TrimSpace(os.Getenv("AI_SCAN_ENDPOINT"))

	// Endpoint env implicitly selects http backend.
	if endpoint != "" && backend == "" {
		backend = "http"
	}

	switch backend {
	case "none":
		return NoopScanner{}, nil
	case "http":
		if endpoint == "" {
			return nil, fmt.Errorf("AI_SCAN_BACKEND=http requires AI_SCAN_ENDPOINT to be set")
		}
		return &HTTPScanner{Endpoint: endpoint}, nil
	case "heuristic", "":
		return &HeuristicScanner{}, nil
	default:
		return nil, fmt.Errorf("unknown AI_SCAN_BACKEND %q (valid: none, http, heuristic)", backend)
	}
}
