package main

import (
	"errors"
	"fmt"
	"sort"
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

type Missing struct{ pkg, dep string }

func (e *Missing) Error() string { return e.pkg + " needs missing " + e.dep }
func (*Missing) depErr()         {}

type Cycle struct{ path []string }

func (e *Cycle) Error() string { return "cycle " + strings.Join(e.path, " -> ") }
func (*Cycle) depErr()         {}

type Stuck struct{ names []string }

func (e *Stuck) Error() string { return "stuck: " + strings.Join(e.names, ", ") }
func (*Stuck) depErr()         {}

func installOrder(reg []Pkg, target string) ([]string, error) {
	if _, err := findPkg(reg, target); err != nil {
		return nil, err
	}
	out := []string{}
	done := map[string]bool{}
	chain := []string{}
	var visit func(name string) error
	visit = func(name string) error {
		for i, c := range chain {
			if c == name {
				path := append([]string{}, chain[i:]...)
				return &Cycle{append(path, name)}
			}
		}
		if done[name] {
			return nil
		}
		p, _ := findPkg(reg, name)
		chain = append(chain, name)
		for _, d := range p.deps {
			if _, err := findPkg(reg, d); err != nil {
				return &Missing{name, d}
			}
			if err := visit(d); err != nil {
				return err
			}
		}
		chain = chain[:len(chain)-1]
		done[name] = true
		out = append(out, name)
		return nil
	}
	if err := visit(target); err != nil {
		return nil, err
	}
	return out, nil
}

func plan(reg []Pkg, target string) string {
	order, err := installOrder(reg, target)
	var u *Unknown
	var m *Missing
	var c *Cycle
	switch {
	case errors.As(err, &u):
		return "error: unknown package " + u.name
	case errors.As(err, &m):
		return "error: " + m.pkg + " needs missing " + m.dep
	case errors.As(err, &c):
		return "error: cycle " + strings.Join(c.path, " -> ")
	case err != nil:
		panic(err)
	}
	return strings.Join(order, " -> ")
}

func fullOrder(reg []Pkg) ([]string, error) {
	installed := map[string]bool{}
	remaining := map[string]Pkg{}
	for _, p := range reg {
		remaining[p.name] = p
	}
	out := []string{}
	for len(remaining) > 0 {
		best := ""
		for n, p := range remaining {
			ok := true
			for _, d := range p.deps {
				if !installed[d] {
					ok = false
					break
				}
			}
			if ok && (best == "" || n < best) {
				best = n
			}
		}
		if best == "" {
			names := []string{}
			for n := range remaining {
				names = append(names, n)
			}
			sort.Strings(names)
			return nil, &Stuck{names}
		}
		installed[best] = true
		delete(remaining, best)
		out = append(out, best)
	}
	return out, nil
}

func dependents(reg []Pkg, name string) ([]string, error) {
	if _, err := findPkg(reg, name); err != nil {
		return nil, err
	}
	seen := map[string]bool{name: true}
	queue := []string{name}
	out := []string{}
	for len(queue) > 0 {
		cur := queue[0]
		queue = queue[1:]
		for _, p := range reg {
			if seen[p.name] {
				continue
			}
			for _, d := range p.deps {
				if d == cur {
					seen[p.name] = true
					out = append(out, p.name)
					queue = append(queue, p.name)
					break
				}
			}
		}
	}
	sort.Strings(out)
	return out, nil
}

func main() {
	fmt.Println(plan(registry(), "app"))
}
