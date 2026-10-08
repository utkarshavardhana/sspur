package main

import (
	"fmt"
	"regexp"
	"slices"
	"sort"
	"strings"
)

type Pair struct {
	word  string
	count int
}

var wordRe = regexp.MustCompile(`[a-z0-9]+`)

func wordsOf(text string, stop []string) []string {
	out := []string{}
	for _, w := range wordRe.FindAllString(strings.ToLower(text), -1) {
		if !slices.Contains(stop, w) {
			out = append(out, w)
		}
	}
	return out
}

func wordFreq(text string, stop []string) []Pair {
	out := []Pair{}
	idx := map[string]int{}
	for _, w := range wordsOf(text, stop) {
		if k, ok := idx[w]; ok {
			out[k].count++
		} else {
			idx[w] = len(out)
			out = append(out, Pair{w, 1})
		}
	}
	return out
}

func topK(text string, k int, stop []string) []Pair {
	if k < 0 {
		panic("k must be >= 0")
	}
	ps := wordFreq(text, stop)
	sort.SliceStable(ps, func(i, j int) bool {
		if ps[i].count != ps[j].count {
			return ps[i].count > ps[j].count
		}
		return ps[i].word < ps[j].word
	})
	return ps[:min(k, len(ps))]
}

func fmtPair(p Pair) string {
	return fmt.Sprintf("%s=%d", p.word, p.count)
}

func topLine(text string, k int, stop []string) string {
	parts := []string{}
	for _, p := range topK(text, k, stop) {
		parts = append(parts, fmtPair(p))
	}
	return strings.Join(parts, " ")
}

func summary(text string, stop []string) string {
	n := len(wordsOf(text, stop))
	d := len(wordFreq(text, stop))
	return fmt.Sprintf("%d words, %d distinct, top: %s", n, d, topLine(text, 3, stop))
}

func longestWord(text string) (string, bool) {
	best, found := "", false
	for _, w := range wordsOf(text, []string{}) {
		if !found || len(w) > len(best) {
			best, found = w, true
		}
	}
	return best, found
}

func main() {
	fmt.Println(summary("The cat and the hat. The end!", []string{}))
}
