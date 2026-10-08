package main

import (
	"reflect"
	"testing"
)

func TestFreqBasic(t *testing.T) {
	if got := wordFreq("a b a"); !reflect.DeepEqual(got, []Pair{{"a", 2}, {"b", 1}}) {
		t.Fatalf("got %v", got)
	}
}

func TestTopOne(t *testing.T) {
	if got := topK("x y y z", 1); !reflect.DeepEqual(got, []Pair{{"y", 2}}) {
		t.Fatalf("got %v", got)
	}
}

func TestSummaryBasic(t *testing.T) {
	if got := summary("b a b"); got != "3 words, 2 distinct, top: b=2 a=1" {
		t.Fatalf("got %q", got)
	}
}
