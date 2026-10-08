package main

import (
	"errors"
	"fmt"
	"slices"
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

type Missing struct {
	pkg string
	dep string
}

func (e *Missing) Error() string { return e.pkg + " needs missing " + e.dep }
func (*Missing) depErr()         {}

type Cycle struct{ path []string }

func (e *Cycle) Error() string { return "cycle " + strings.Join(e.path, " -> ") }
func (*Cycle) depErr()         {}

type Stuck struct{ names []string }

func (e *Stuck) Error() string { return "stuck " + strings.Join(e.names, ", ") }
func (*Stuck) depErr()         {}

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

func hasPkg(reg []Pkg, name string) bool {
	_, err := findPkg(reg, name)
	return err == nil
}

func visit(reg []Pkg, name string, chain []string, done *[]string) error {
	if slices.Contains(*done, name) {
		return nil
	}
	if at := slices.Index(chain, name); at >= 0 {
		path := append(append([]string{}, chain[at:]...), name)
		return &Cycle{path}
	}
	p, err := findPkg(reg, name)
	if err != nil {
		return err
	}
	next := append(append([]string{}, chain...), name)
	for _, d := range p.deps {
		if !hasPkg(reg, d) {
			return &Missing{name, d}
		}
		if err := visit(reg, d, next, done); err != nil {
			return err
		}
	}
	*done = append(*done, name)
	return nil
}

func installOrder(reg []Pkg, target string) ([]string, error) {
	done := []string{}
	if err := visit(reg, target, []string{}, &done); err != nil {
		return nil, err
	}
	return done, nil
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
		return fmt.Sprintf("error: %s needs missing %s", m.pkg, m.dep)
	case errors.As(err, &c):
		return "error: cycle " + strings.Join(c.path, " -> ")
	case err != nil:
		panic(err)
	}
	return strings.Join(order, " -> ")
}

func fullOrder(reg []Pkg) ([]string, error) {
	done := []string{}
	for {
		ready := []string{}
		for _, p := range reg {
			if slices.Contains(done, p.name) {
				continue
			}
			all := true
			for _, d := range p.deps {
				if !slices.Contains(done, d) {
					all = false
				}
			}
			if all {
				ready = append(ready, p.name)
			}
		}
		if len(ready) == 0 {
			break
		}
		sort.Strings(ready)
		done = append(done, ready[0])
	}
	if len(done) < len(reg) {
		left := []string{}
		for _, p := range reg {
			if !slices.Contains(done, p.name) {
				left = append(left, p.name)
			}
		}
		sort.Strings(left)
		return nil, &Stuck{left}
	}
	return done, nil
}

func dependents(reg []Pkg, name string) ([]string, error) {
	if _, err := findPkg(reg, name); err != nil {
		return nil, err
	}
	found := map[string]bool{name: true}
	changed := true
	for changed {
		changed = false
		for _, p := range reg {
			if found[p.name] {
				continue
			}
			for _, d := range p.deps {
				if found[d] {
					found[p.name] = true
					changed = true
					break
				}
			}
		}
	}
	out := []string{}
	for n := range found {
		if n != name {
			out = append(out, n)
		}
	}
	sort.Strings(out)
	return out, nil
}

func main() {
	fmt.Println(plan(registry(), "app"))
}
