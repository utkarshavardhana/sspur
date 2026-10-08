package main

import (
	"fmt"
	"sort"
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

func sorted(xs []string) []string {
	c := append([]string{}, xs...)
	sort.Strings(c)
	return c
}

func look(g Game) string {
	r := current(g)
	dirs := []string{}
	for _, e := range r.exits {
		dirs = append(dirs, e.dir)
	}
	out := fmt.Sprintf("You are in the %s. Exits: %s.", r.name, strings.Join(sorted(dirs), ", "))
	if len(r.items) > 0 {
		out = fmt.Sprintf("%s Items: %s.", out, strings.Join(sorted(r.items), ", "))
	}
	return out
}

func indexOf(xs []string, x string) int {
	for i, v := range xs {
		if v == x {
			return i
		}
	}
	return -1
}

func without(xs []string, i int) []string {
	c := append([]string{}, xs[:i]...)
	return append(c, xs[i+1:]...)
}

func setRoomItems(g Game, items []string) Game {
	rooms := append([]Room{}, g.rooms...)
	for i := range rooms {
		if rooms[i].name == g.room {
			rooms[i].items = items
		}
	}
	g.rooms = rooms
	return g
}

func goDir(g Game, dir string) (Game, string) {
	r := current(g)
	for _, e := range r.exits {
		if e.dir == dir {
			if indexOf(r.locked, dir) >= 0 && indexOf(g.inventory, "key") < 0 {
				return g, "The door is locked."
			}
			g.room = e.target
			g.moves++
			return g, look(g)
		}
	}
	return g, "You can't go that way."
}

func score(g Game) int {
	s := 10 * len(g.inventory)
	if g.room == "vault" {
		s += 25
	}
	return s
}

func step(g Game, cmd string) (Game, string) {
	w := strings.Fields(strings.ToLower(cmd))
	if len(w) == 0 {
		return g, "I don't understand."
	}
	switch {
	case w[0] == "look" && len(w) == 1:
		return g, look(g)
	case w[0] == "go" && len(w) == 2:
		return goDir(g, w[1])
	case w[0] == "inventory" && len(w) == 1:
		if len(g.inventory) == 0 {
			return g, "You carry nothing."
		}
		return g, "You carry: " + strings.Join(sorted(g.inventory), ", ") + "."
	case w[0] == "take" && len(w) == 2:
		items := current(g).items
		i := indexOf(items, w[1])
		if i < 0 {
			return g, "There is no " + w[1] + " here."
		}
		g = setRoomItems(g, without(items, i))
		g.inventory = append(append([]string{}, g.inventory...), w[1])
		return g, "Taken."
	case w[0] == "drop" && len(w) == 2:
		i := indexOf(g.inventory, w[1])
		if i < 0 {
			return g, "You don't have " + w[1] + "."
		}
		g.inventory = without(g.inventory, i)
		g = setRoomItems(g, append(append([]string{}, current(g).items...), w[1]))
		return g, "Dropped."
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

func main() {
	for _, line := range play([]string{"look", "go east", "go west"}) {
		fmt.Println(line)
	}
}
