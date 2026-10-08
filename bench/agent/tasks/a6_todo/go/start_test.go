package main

import (
	"reflect"
	"testing"
)

func ids(ts []Task) []int {
	out := []int{}
	for _, t := range ts {
		out = append(out, t.id)
	}
	return out
}

func TestIds(t *testing.T) {
	if got := ids(demo()); !reflect.DeepEqual(got, []int{1, 2, 3, 4}) {
		t.Fatalf("got %v", got)
	}
}

func TestCompleteOne(t *testing.T) {
	if got := ids(pending(complete(demo(), 2))); !reflect.DeepEqual(got, []int{1, 3, 4}) {
		t.Fatalf("got %v", got)
	}
}

func TestRenderOne(t *testing.T) {
	if got := render(Task{5, "nap", true, Low}); got != "[x] #5 nap" {
		t.Fatalf("got %q", got)
	}
}
