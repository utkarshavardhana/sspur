package main

import (
	"reflect"
	"testing"
)

func TestStartsInHall(t *testing.T) {
	if got := play([]string{"look"}); !reflect.DeepEqual(got, []string{"You are in the hall. Exits: north, east."}) {
		t.Fatalf("got %q", got)
	}
}

func TestWalks(t *testing.T) {
	got := play([]string{"go east", "go west"})
	if got[len(got)-1] != "You are in the hall. Exits: north, east." {
		t.Fatalf("got %q", got)
	}
}

func TestBlocked(t *testing.T) {
	if got := play([]string{"go up"}); !reflect.DeepEqual(got, []string{"You can't go that way."}) {
		t.Fatalf("got %q", got)
	}
}

func TestCountsMoves(t *testing.T) {
	if g, _ := run([]string{"go east", "go west"}); g.moves != 2 {
		t.Fatalf("moves = %d", g.moves)
	}
}
