package aiscan

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net/http"
)

// HTTPScanner POSTs file content to an external AI-detection service and
// interprets its JSON response.
//
// Request body (JSON):
//
//	{"path": "<file path>", "content": "<utf-8 text>"}
//
// Expected response body (JSON):
//
//	{"likely_ai": <bool>, "confidence": <float 0-1>}
//
// Any non-2xx response is treated as a scan error (not a positive detection).
type HTTPScanner struct {
	// Endpoint is the full URL to POST to (from AI_SCAN_ENDPOINT).
	Endpoint string
}

type httpRequest struct {
	Path    string `json:"path"`
	Content string `json:"content"`
}

type httpResponse struct {
	LikelyAI   bool    `json:"likely_ai"`
	Confidence float64 `json:"confidence"`
}

func (s *HTTPScanner) Scan(path string, content []byte) (bool, float64, error) {
	body, err := json.Marshal(httpRequest{
		Path:    path,
		Content: string(content),
	})
	if err != nil {
		return false, 0, fmt.Errorf("aiscan/http: marshal request: %w", err)
	}

	resp, err := http.Post(s.Endpoint, "application/json", bytes.NewReader(body)) //nolint:noctx
	if err != nil {
		return false, 0, fmt.Errorf("aiscan/http: POST %s: %w", s.Endpoint, err)
	}
	defer resp.Body.Close()

	if resp.StatusCode < 200 || resp.StatusCode >= 300 {
		return false, 0, fmt.Errorf("aiscan/http: server returned %s", resp.Status)
	}

	var result httpResponse
	if err := json.NewDecoder(resp.Body).Decode(&result); err != nil {
		return false, 0, fmt.Errorf("aiscan/http: decode response: %w", err)
	}

	return result.LikelyAI, result.Confidence, nil
}
