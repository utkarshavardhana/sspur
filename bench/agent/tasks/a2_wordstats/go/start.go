package main

import (
	"fmt"
	"regexp"
	"sort"
	"strings"
)

type Pair struct {
	word  string
	count int
}

var wordRe = regexp.MustCompile(`[a-z0-9]+`)

func wordsOf(text string) []string {
	out := []string{}
	for _, w := range wordRe.FindAllString(strings.ToLower(text), -1) {
		out = append(out, w)
	}
	return out
}

func wordFreq(text string) []Pair {
	out := []Pair{}
	idx := map[string]int{}
	for _, w := range wordsOf(text) {
		if k, ok := idx[w]; ok {
			out[k].count++
		} else {
			idx[w] = len(out)
			out = append(out, Pair{w, 1})
		}
	}
	return out
}

func topK(text string, k int) []Pair {
	if k < 0 {
		panic("k must be >= 0")
	}
	ps := wordFreq(text)
	sort.SliceStable(ps, func(i, j int) bool { return ps[i].count > ps[j].count })
	return ps[:min(k, len(ps))]
}

func fmtPair(p Pair) string {
	return fmt.Sprintf("%s=%d", p.word, p.count)
}

func topLine(text string, k int) string {
	parts := []string{}
	for _, p := range topK(text, k) {
		parts = append(parts, fmtPair(p))
	}
	return strings.Join(parts, " ")
}

func summary(text string) string {
	n := len(wordsOf(text))
	d := len(wordFreq(text))
	return fmt.Sprintf("%d words, %d distinct, top: %s", n, d, topLine(text, 3))
}

func main() {
	fmt.Println(summary("The cat and the hat. The end!"))
}
