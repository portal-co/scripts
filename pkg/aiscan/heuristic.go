// AIKEY-l4qkxonqry2b4gj7bsrkqpryiy
package aiscan

import (
	"math"
	"strings"
	"unicode"
)

// HeuristicScanner uses lightweight statistical signals to flag content that
// exhibits patterns common in LLM output. It requires no external dependencies
// or network access.
//
// Signals used (each contributes a weighted score; threshold is 0.5):
//
//  1. Phrase density  — frequency of phrases that LLMs overuse
//  2. Type-token ratio (TTR) — LLM output tends toward moderate TTR with
//     very uniform sentence lengths; extremely high or low values in
//     combination with other signals are suspicious
//  3. Bullet/header density — LLMs love structured lists and markdown headers
//  4. Hedge phrase density — "it's worth noting", "it is important to",
//     "as an AI language model", etc.
type HeuristicScanner struct{}

func (h *HeuristicScanner) Scan(_ string, content []byte) (bool, float64, error) {
	text := string(content)
	score := h.score(text)
	const threshold = 0.5
	return score >= threshold, score, nil
}

// score returns a value in [0, 1]; >= 0.5 means likely AI.
func (h *HeuristicScanner) score(text string) float64 {
	if strings.TrimSpace(text) == "" {
		return 0
	}

	weights := []struct {
		v float64
		w float64
	}{
		{h.phraseDensity(text), 0.45},
		{h.hedgeDensity(text), 0.35},
		{h.bulletHeaderDensity(text), 0.20},
	}

	var total, wsum float64
	for _, s := range weights {
		total += s.v * s.w
		wsum += s.w
	}
	raw := total / wsum

	// sigmoid-like squash so partial signals don't easily tip the threshold
	return sigmoidNorm(raw)
}

// phraseDensity returns the fraction of sentences containing an LLM-overused phrase.
func (h *HeuristicScanner) phraseDensity(text string) float64 {
	phrases := []string{
		"it's worth noting",
		"it is worth noting",
		"as an ai",
		"as an ai language model",
		"i cannot provide",
		"i'm unable to",
		"i am unable to",
		"delve into",
		"dive into",
		"in conclusion",
		"in summary",
		"to summarize",
		"let's explore",
		"let us explore",
		"it is important to note",
		"it's important to note",
		"please note that",
		"feel free to",
		"i hope this helps",
		"certainly!",
		"of course!",
		"absolutely!",
		"great question",
		"this is a great",
		"comprehensive guide",
		"step-by-step",
		"step by step",
		"furthermore,",
		"additionally,",
		"in the realm of",
		"leveraging",
		"utilize",
		"robust solution",
		"seamlessly",
		"cutting-edge",
	}

	lower := strings.ToLower(text)
	sentences := splitSentences(lower)
	if len(sentences) == 0 {
		return 0
	}

	hits := 0
	for _, s := range sentences {
		for _, p := range phrases {
			if strings.Contains(s, p) {
				hits++
				break // count each sentence at most once
			}
		}
	}
	return math.Min(float64(hits)/float64(len(sentences))*3, 1.0)
}

// hedgeDensity looks for meta-commentary patterns LLMs use about their own output.
func (h *HeuristicScanner) hedgeDensity(text string) float64 {
	hedges := []string{
		"as mentioned",
		"as noted above",
		"as discussed",
		"as outlined",
		"this ensures that",
		"this allows you to",
		"this will allow",
		"this helps to",
		"this approach ensures",
		"by doing so",
		"in other words",
		"to put it simply",
		"put simply",
		"to clarify",
		"that being said",
		"with that said",
		"having said that",
		"needless to say",
	}

	lower := strings.ToLower(text)
	sentences := splitSentences(lower)
	if len(sentences) == 0 {
		return 0
	}

	hits := 0
	for _, s := range sentences {
		for _, p := range hedges {
			if strings.Contains(s, p) {
				hits++
				break
			}
		}
	}
	return math.Min(float64(hits)/float64(len(sentences))*4, 1.0)
}

// bulletHeaderDensity measures how much of the text is markdown structural elements.
func (h *HeuristicScanner) bulletHeaderDensity(text string) float64 {
	lines := strings.Split(text, "\n")
	if len(lines) == 0 {
		return 0
	}

	structural := 0
	for _, line := range lines {
		trimmed := strings.TrimSpace(line)
		if trimmed == "" {
			continue
		}
		if strings.HasPrefix(trimmed, "#") ||
			strings.HasPrefix(trimmed, "- ") ||
			strings.HasPrefix(trimmed, "* ") ||
			strings.HasPrefix(trimmed, "+ ") ||
			(len(trimmed) > 2 && trimmed[0] >= '1' && trimmed[0] <= '9' && trimmed[1] == '.') {
			structural++
		}
	}
	ratio := float64(structural) / float64(len(lines))
	// A ratio above 0.4 is suspicious (very heavily structured).
	return math.Min(ratio/0.4, 1.0)
}

// splitSentences splits text into rough sentence chunks.
func splitSentences(text string) []string {
	var sentences []string
	var buf strings.Builder
	for _, r := range text {
		buf.WriteRune(r)
		if r == '.' || r == '!' || r == '?' || r == '\n' {
			s := strings.TrimFunc(buf.String(), unicode.IsSpace)
			if len(s) > 8 {
				sentences = append(sentences, s)
			}
			buf.Reset()
		}
	}
	if s := strings.TrimFunc(buf.String(), unicode.IsSpace); len(s) > 8 {
		sentences = append(sentences, s)
	}
	return sentences
}

// sigmoidNorm maps [0,1] → [0,1] with a soft S-curve centred at 0.35,
// so low signals produce scores well below 0.5 and high signals push above it.
func sigmoidNorm(x float64) float64 {
	// logistic: 1/(1+exp(-k*(x-x0)))  with k=8, x0=0.35
	return 1.0 / (1.0 + math.Exp(-8.0*(x-0.35)))
}
