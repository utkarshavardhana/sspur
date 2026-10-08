package main

import (
	"reflect"
	"testing"
)

func TestLeaf(t *testing.T) {
	got, err := installOrder(registry(), "http")
	if err != nil || !reflect.DeepEqual(got, []string{"http"}) {
		t.Fatalf("got %v, %v", got, err)
	}
}

func TestDbFirstLog(t *testing.T) {
	got, err := installOrder(registry(), "db")
	if err != nil || !reflect.DeepEqual(got, []string{"log", "db"}) {
		t.Fatalf("got %v, %v", got, err)
	}
}

func TestUnknown(t *testing.T) {
	if got := plan(registry(), "nope"); got != "error: unknown package nope" {
		t.Fatalf("got %q", got)
	}
}
