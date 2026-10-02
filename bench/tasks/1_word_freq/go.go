package words

import (
	"regexp"
	"sort"
	"strings"
)

var wordRe = regexp.MustCompile(`[a-z]+`)

type WordCount struct {
	Word  string
	Count int
}

func TopWords(text string, k int) []WordCount {
	counts := map[string]int{}
	for _, w := range wordRe.FindAllString(strings.ToLower(text), -1) {
		counts[w]++
	}
	out := make([]WordCount, 0, len(counts))
	for w, c := range counts {
		out = append(out, WordCount{w, c})
	}
	sort.Slice(out, func(i, j int) bool {
		if out[i].Count != out[j].Count {
			return out[i].Count > out[j].Count
		}
		return out[i].Word < out[j].Word
	})
	if len(out) > k {
		out = out[:k]
	}
	return out
}
