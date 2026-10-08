package main

import (
	"errors"
	"fmt"
	"strings"
)

type Pkg struct {
	name string
	deps []string
}

// DepErr is implemented by every resolver error.
type DepErr interface {
	error
	depErr()
}

type Unknown struct{ name string }

func (e *Unknown) Error() string { return "unknown package " + e.name }
func (*Unknown) depErr()         {}

func registry() []Pkg {
	return []Pkg{{"app", []string{"web", "db"}}, {"web", []string{"http", "log"}}, {"db", []string{"log"}}, {"http", []string{}}, {"log", []string{}}}
}

func findPkg(reg []Pkg, name string) (Pkg, error) {
	for _, p := range reg {
		if p.name == name {
			return p, nil
		}
	}
	return Pkg{}, &Unknown{name}
}

func installOrder(reg []Pkg, target string) ([]string, error) {
	p, err := findPkg(reg, target)
	if err != nil {
		return nil, err
	}
	out := []string{}
	for _, d := range p.deps {
		sub, err := installOrder(reg, d)
		if err != nil {
			return nil, err
		}
		out = append(out, sub...)
	}
	return append(out, target), nil
}

func plan(reg []Pkg, target string) string {
	order, err := installOrder(reg, target)
	var u *Unknown
	if errors.As(err, &u) {
		return "error: unknown package " + u.name
	}
	if err != nil {
		panic(err)
	}
	return strings.Join(order, " -> ")
}

func main() {
	fmt.Println(plan(registry(), "app"))
}
