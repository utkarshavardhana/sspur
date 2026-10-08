package main

import (
	"reflect"
	"testing"
)

const hHall = "You are in the hall. Exits: east, north."
const hKitchen = "You are in the kitchen. Exits: west. Items: bread, key."
const hNope = "I don't understand."

func hPlay(t *testing.T, cmds []string, from int, want []string) {
	t.Helper()
	got := play(cmds)
	if from > len(got) || !reflect.DeepEqual(got[from:], want) {
		t.Fatalf("play(%q) = %q, want %q from %d", cmds, got, want, from)
	}
}

func hLast(t *testing.T, cmds []string, want string) {
	t.Helper()
	got := play(cmds)
	if len(got) == 0 || got[len(got)-1] != want {
		t.Fatalf("play(%q) = %q, want last %q", cmds, got, want)
	}
}

func hGame(cmds []string) Game {
	g, _ := run(cmds)
	return g
}

func TestHidden_look_sorted(t *testing.T) {
	hPlay(t, []string{"look"}, 0, []string{hHall})
}

func TestHidden_items_shown(t *testing.T) {
	hPlay(t, []string{"go east"}, 0, []string{hKitchen})
}

func TestHidden_case_spaces(t *testing.T) {
	hPlay(t, []string{"  GO   East ", "LOOK", "Look  "}, 0, []string{hKitchen, hKitchen, hKitchen})
}

func TestHidden_unknown(t *testing.T) {
	hPlay(t, []string{"dance", "", "go", "go east west"}, 0, []string{hNope, hNope, hNope, hNope})
}

func TestHidden_no_exit(t *testing.T) {
	hPlay(t, []string{"go west"}, 0, []string{"You can't go that way."})
}

func TestHidden_take(t *testing.T) {
	hPlay(t, []string{"go east", "take key", "look"}, 0, []string{hKitchen, "Taken.", "You are in the kitchen. Exits: west. Items: bread."})
}

func TestHidden_take_case(t *testing.T) {
	hPlay(t, []string{"go east", "TAKE Key", "inventory"}, 1, []string{"Taken.", "You carry: key."})
}

func TestHidden_take_missing(t *testing.T) {
	hPlay(t, []string{"take key"}, 0, []string{"There is no key here."})
}

func TestHidden_inventory_empty(t *testing.T) {
	hPlay(t, []string{"inventory"}, 0, []string{"You carry nothing."})
}

func TestHidden_inventory_sorted(t *testing.T) {
	hLast(t, []string{"go east", "take key", "take bread", "inventory"}, "You carry: bread, key.")
}

func TestHidden_drop(t *testing.T) {
	hPlay(t, []string{"go east", "take key", "go west", "drop key", "look", "inventory"}, 2,
		[]string{hHall, "Dropped.", "You are in the hall. Exits: east, north. Items: key.", "You carry nothing."})
}

func TestHidden_drop_missing(t *testing.T) {
	hPlay(t, []string{"drop gold"}, 0, []string{"You don't have gold."})
}

func TestHidden_locked(t *testing.T) {
	hPlay(t, []string{"go north"}, 0, []string{"The door is locked."})
	if g := hGame([]string{"go north"}); g.room != "hall" {
		t.Fatalf("room = %q", g.room)
	}
}

func TestHidden_unlocked(t *testing.T) {
	hLast(t, []string{"go east", "take key", "go west", "go north"}, "You are in the vault. Exits: south. Items: gold.")
}

func TestHidden_moves(t *testing.T) {
	if g := hGame([]string{"go east", "go up", "go west", "go north"}); g.moves != 2 {
		t.Fatalf("moves = %d", g.moves)
	}
}

func TestHidden_world_locked(t *testing.T) {
	got := [][]string{}
	for _, r := range world() {
		got = append(got, append([]string{}, r.locked...))
	}
	if !reflect.DeepEqual(got, [][]string{{"north"}, {}, {}}) {
		t.Fatalf("got %q", got)
	}
}

func TestHidden_score(t *testing.T) {
	if s := score(newGame()); s != 0 {
		t.Fatalf("new game score = %d", s)
	}
	if s := score(hGame([]string{"go east", "take key", "take bread"})); s != 20 {
		t.Fatalf("two items score = %d", s)
	}
	if s := score(hGame([]string{"go east", "take key", "go west", "go north", "take gold"})); s != 45 {
		t.Fatalf("vault score = %d", s)
	}
}
