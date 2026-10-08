package main

import (
	"errors"
	"testing"
)

func TestParseSample(t *testing.T) {
	if got := len(parse(sampleText())); got != 3 {
		t.Fatalf("got %d", got)
	}
}

func TestLookupHost(t *testing.T) {
	if v, ok := lookup(parse(sampleText()), "host"); !ok || v != "localhost" {
		t.Fatalf("got %q, %v", v, ok)
	}
}

func TestMissing(t *testing.T) {
	_, err := getStr(parse(sampleText()), "nope")
	var e *Missing
	if !errors.As(err, &e) || e.key != "nope" {
		t.Fatalf("err = %v", err)
	}
}
