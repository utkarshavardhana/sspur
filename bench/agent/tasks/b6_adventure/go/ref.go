package main

import (
	"fmt"
	"slices"
	"strings"
)

type Exit struct {
	dir    string
	target string
}

type Room struct {
	name   string
	exits  []Exit
	items  []string
	locked []string
}

type Game struct {
	room      string
	rooms     []Room
	inventory []string
	moves     int
}

func world() []Room {
	return []Room{
		{"hall", []Exit{{"north", "vault"}, {"east", "kitchen"}}, []string{}, []string{"north"}},
		{"kitchen", []Exit{{"west", "hall"}}, []string{"key", "bread"}, []string{}},
		{"vault", []Exit{{"south", "hall"}}, []string{"gold"}, []string{}},
	}
}

func newGame() Game {
	return Game{"hall", world(), []string{}, 0}
}

func current(g Game) Room {
	for _, r := range g.rooms {
		if r.name == g.room {
			return r
		}
	}
	panic("no room " + g.room)
}

func sortedCopy(xs []string) []string {
	out := append([]string{}, xs...)
	slices.Sort(out)
	return out
}

func look(g Game) string {
	r := current(g)
	dirs := []string{}
	for _, e := range r.exits {
		dirs = append(dirs, e.dir)
	}
	items := ""
	if len(r.items) > 0 {
		items = " Items: " + strings.Join(sortedCopy(r.items), ", ") + "."
	}
	return fmt.Sprintf("You are in the %s. Exits: %s.%s", r.name, strings.Join(sortedCopy(dirs), ", "), items)
}

func goDir(g Game, dir string) (Game, string) {
	for _, e := range current(g).exits {
		if e.dir == dir {
			if slices.Contains(current(g).locked, dir) && !slices.Contains(g.inventory, "key") {
				return g, "The door is locked."
			}
			g.room = e.target
			g.moves++
			return g, look(g)
		}
	}
	return g, "You can't go that way."
}

func without(xs []string, x string) []string {
	out := []string{}
	for _, v := range xs {
		if v != x {
			out = append(out, v)
		}
	}
	return out
}

func setItems(g Game, items []string) Game {
	rooms := make([]Room, len(g.rooms))
	for k, r := range g.rooms {
		if r.name == g.room {
			r.items = items
		}
		rooms[k] = r
	}
	g.rooms = rooms
	return g
}

func take(g Game, item string) (Game, string) {
	if slices.Contains(current(g).items, item) {
		g2 := setItems(g, without(current(g).items, item))
		g2.inventory = append(append([]string{}, g.inventory...), item)
		return g2, "Taken."
	}
	return g, "There is no " + item + " here."
}

func drop(g Game, item string) (Game, string) {
	if slices.Contains(g.inventory, item) {
		g2 := setItems(g, append(append([]string{}, current(g).items...), item))
		g2.inventory = without(g.inventory, item)
		return g2, "Dropped."
	}
	return g, "You don't have " + item + "."
}

func step(g Game, cmd string) (Game, string) {
	w := strings.Fields(strings.ToLower(cmd))
	switch {
	case len(w) == 1 && w[0] == "look":
		return g, look(g)
	case len(w) == 1 && w[0] == "inventory":
		if len(g.inventory) == 0 {
			return g, "You carry nothing."
		}
		return g, "You carry: " + strings.Join(sortedCopy(g.inventory), ", ") + "."
	case len(w) == 2 && w[0] == "go":
		return goDir(g, w[1])
	case len(w) == 2 && w[0] == "take":
		return take(g, w[1])
	case len(w) == 2 && w[0] == "drop":
		return drop(g, w[1])
	}
	return g, "I don't understand."
}

func run(cmds []string) (Game, []string) {
	g := newGame()
	out := []string{}
	for _, c := range cmds {
		var msg string
		g, msg = step(g, c)
		out = append(out, msg)
	}
	return g, out
}

func play(cmds []string) []string {
	_, out := run(cmds)
	return out
}

func score(g Game) int {
	s := 10 * len(g.inventory)
	if g.room == "vault" {
		s += 25
	}
	return s
}

func main() {
	for _, line := range play([]string{"look", "go east", "go west"}) {
		fmt.Println(line)
	}
}
