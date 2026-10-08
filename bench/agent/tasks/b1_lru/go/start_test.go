package main

import (
	"reflect"
	"testing"
)

func TestPutTwo(t *testing.T) {
	if got := keys(put(put(newCache(2), "a", 1), "b", 2)); !reflect.DeepEqual(got, []string{"a", "b"}) {
		t.Fatalf("got %v", got)
	}
}

func TestLookupHit(t *testing.T) {
	if v, ok := lookup(put(newCache(2), "a", 1), "a"); !ok || v != 1 {
		t.Fatalf("got %d %v", v, ok)
	}
}

func TestLookupMiss(t *testing.T) {
	if got := getOr(newCache(2), "z", -1); got != -1 {
		t.Fatalf("got %d", got)
	}
}

func TestDescribed(t *testing.T) {
	if got := describe(put(newCache(3), "x", 7)); got != "cap=3 [x=7]" {
		t.Fatalf("got %q", got)
	}
}
