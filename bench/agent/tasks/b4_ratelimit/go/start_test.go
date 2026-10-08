package main

import "testing"

func TestFirstAllowed(t *testing.T) {
	if ok, _ := allow(newWindow(3, 10), 0); !ok {
		t.Fatal("not allowed")
	}
}

func TestCounts(t *testing.T) {
	if _, w := allow(newWindow(3, 10), 0); w.count != 1 {
		t.Fatalf("count = %d", w.count)
	}
}

func TestNewWindowResets(t *testing.T) {
	got := runRequests(newWindow(1, 10), []int{0, 10})
	if !got[len(got)-1] {
		t.Fatalf("got %v", got)
	}
}
