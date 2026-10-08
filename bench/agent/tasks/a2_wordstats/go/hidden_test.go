package main

import (
	"reflect"
	"testing"
)

const hText = "the cat and the dog and the bird"

func hPairs(t *testing.T, got, want []Pair) {
	t.Helper()
	if len(got) == 0 && len(want) == 0 {
		return
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("got %v, want %v", got, want)
	}
}

func TestHidden_ties(t *testing.T) {
	hPairs(t, topK(hText, 3, []string{}), []Pair{{"the", 3}, {"and", 2}, {"bird", 1}})
}

func TestHidden_ties_all(t *testing.T) {
	hPairs(t, topK("d c b a c", 4, []string{}), []Pair{{"c", 2}, {"a", 1}, {"b", 1}, {"d", 1}})
}

func TestHidden_stop_top(t *testing.T) {
	hPairs(t, topK(hText, 2, []string{"the", "and"}), []Pair{{"bird", 1}, {"cat", 1}})
}

func TestHidden_stop_freq(t *testing.T) {
	hPairs(t, wordFreq("The THE cat", []string{"the"}), []Pair{{"cat", 1}})
}

func TestHidden_words_stop(t *testing.T) {
	if got := wordsOf("A b, a C!", []string{"a"}); !reflect.DeepEqual(got, []string{"b", "c"}) {
		t.Fatalf("got %v", got)
	}
}

func TestHidden_summary_stop(t *testing.T) {
	if got := summary(hText, []string{"the"}); got != "5 words, 4 distinct, top: and=2 bird=1 cat=1" {
		t.Fatalf("got %q", got)
	}
}

func TestHidden_summary_nostop(t *testing.T) {
	if got := summary("b a b", []string{}); got != "3 words, 2 distinct, top: b=2 a=1" {
		t.Fatalf("got %q", got)
	}
}

func TestHidden_top_line(t *testing.T) {
	if got := topLine("q q r", 5, []string{"r"}); got != "q=2" {
		t.Fatalf("got %q", got)
	}
}

func TestHidden_longest(t *testing.T) {
	if got, ok := longestWord("a bb ccc dd eee"); !ok || got != "ccc" {
		t.Fatalf("got %q %v", got, ok)
	}
}

func TestHidden_longest_case(t *testing.T) {
	if got, ok := longestWord("Hello World"); !ok || got != "hello" {
		t.Fatalf("got %q %v", got, ok)
	}
}

func TestHidden_longest_none(t *testing.T) {
	if got, ok := longestWord("!! ??"); ok {
		t.Fatalf("got %q, want none", got)
	}
}
