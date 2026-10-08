package main

import (
	"fmt"
	"strings"
)

type Exit struct {
	dir    string
	target string
}

type Room struct {
	name  string
	exits []Exit
	items []string
}

type Game struct {
	room      string
	rooms     []Room
	inventory []string
	moves     int
}

func world() []Room {
	return []Room{
		{"hall", []Exit{{"north", "vault"}, {"east", "kitchen"}}, []string{}},
		{"kitchen", []Exit{{"west", "hall"}}, []string{"key", "bread"}},
		{"vault", []Exit{{"south", "hall"}}, []string{"gold"}},
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

func look(g Game) string {
	r := current(g)
	dirs := []string{}
	for _, e := range r.exits {
		dirs = append(dirs, e.dir)
	}
	return fmt.Sprintf("You are in the %s. Exits: %s.", r.name, strings.Join(dirs, ", "))
}

func goDir(g Game, dir string) (Game, string) {
	for _, e := range current(g).exits {
		if e.dir == dir {
			g.room = e.target
			g.moves++
			return g, look(g)
		}
	}
	return g, "You can't go that way."
}

func step(g Game, cmd string) (Game, string) {
	w := strings.Split(cmd, " ")
	if w[0] == "look" {
		return g, look(g)
	}
	if w[0] == "go" && len(w) == 2 {
		return goDir(g, w[1])
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
