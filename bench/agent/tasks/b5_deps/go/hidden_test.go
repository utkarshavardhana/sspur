package main

import (
	"errors"
	"strings"
	"testing"
)

func hOrder(reg []Pkg, target string) string {
	order, err := installOrder(reg, target)
	if err == nil {
		return strings.Join(order, ",")
	}
	var u *Unknown
	var m *Missing
	var c *Cycle
	var de DepErr
	switch {
	case errors.As(err, &u):
		return "unknown " + u.name
	case errors.As(err, &m):
		return "missing " + m.pkg + " " + m.dep
	case errors.As(err, &c):
		return "cycle " + strings.Join(c.path, ",")
	case errors.As(err, &de):
		return "other"
	}
	return "unexpected " + err.Error()
}

func hFull(reg []Pkg) string {
	order, err := fullOrder(reg)
	if err == nil {
		return strings.Join(order, ",")
	}
	var s *Stuck
	var de DepErr
	switch {
	case errors.As(err, &s):
		return "stuck " + strings.Join(s.names, ",")
	case errors.As(err, &de):
		return "other"
	}
	return "unexpected " + err.Error()
}

func hDeps(reg []Pkg, name string) string {
	ds, err := dependents(reg, name)
	if err == nil {
		return strings.Join(ds, ",")
	}
	var u *Unknown
	var de DepErr
	switch {
	case errors.As(err, &u):
		return "unknown " + u.name
	case errors.As(err, &de):
		return "other"
	}
	return "unexpected " + err.Error()
}

func hDiamond() []Pkg {
	return []Pkg{{"a", []string{"b", "c"}}, {"b", []string{"d"}}, {"c", []string{"d"}}, {"d", []string{}}}
}

func hEq(t *testing.T, got, want string) {
	t.Helper()
	if got != want {
		t.Fatalf("got %q, want %q", got, want)
	}
}

func TestHidden_once(t *testing.T) {
	hEq(t, hOrder(registry(), "app"), "http,log,web,db,app")
}

func TestHidden_leaf(t *testing.T) {
	hEq(t, hOrder(registry(), "log"), "log")
}

func TestHidden_diamond(t *testing.T) {
	hEq(t, hOrder(hDiamond(), "a"), "d,b,c,a")
}

func TestHidden_unknown(t *testing.T) {
	hEq(t, hOrder(registry(), "zzz"), "unknown zzz")
}

func TestHidden_missing(t *testing.T) {
	hEq(t, hOrder([]Pkg{{"a", []string{"b"}}, {"b", []string{"x"}}}, "a"), "missing b x")
}

func TestHidden_cycle(t *testing.T) {
	hEq(t, hOrder([]Pkg{{"a", []string{"b"}}, {"b", []string{"c"}}, {"c", []string{"a"}}}, "a"), "cycle a,b,c,a")
}

func TestHidden_self_cycle(t *testing.T) {
	hEq(t, hOrder([]Pkg{{"a", []string{"a"}}}, "a"), "cycle a,a")
}

func TestHidden_deep_cycle(t *testing.T) {
	hEq(t, hOrder([]Pkg{{"top", []string{"b"}}, {"b", []string{"c"}}, {"c", []string{"b"}}}, "top"), "cycle b,c,b")
}

func TestHidden_full(t *testing.T) {
	hEq(t, hFull(registry()), "http,log,db,web,app")
}

func TestHidden_full_diamond(t *testing.T) {
	hEq(t, hFull(hDiamond()), "d,b,c,a")
}

func TestHidden_full_stuck(t *testing.T) {
	hEq(t, hFull([]Pkg{{"a", []string{"b"}}, {"b", []string{"a"}}, {"c", []string{}}}), "stuck a,b")
}

func TestHidden_full_missing(t *testing.T) {
	hEq(t, hFull([]Pkg{{"a", []string{"x"}}, {"b", []string{}}}), "stuck a")
}

func TestHidden_full_empty(t *testing.T) {
	hEq(t, hFull([]Pkg{}), "")
}

func TestHidden_dependents(t *testing.T) {
	hEq(t, hDeps(registry(), "log"), "app,db,web")
	hEq(t, hDeps(registry(), "http"), "app,web")
}

func TestHidden_dependents_none(t *testing.T) {
	hEq(t, hDeps(registry(), "app"), "")
	hEq(t, hDeps(hDiamond(), "d"), "a,b,c")
}

func TestHidden_dependents_unknown(t *testing.T) {
	hEq(t, hDeps(registry(), "zzz"), "unknown zzz")
}

func TestHidden_plan(t *testing.T) {
	hEq(t, plan(registry(), "app"), "http -> log -> web -> db -> app")
}

func TestHidden_plan_errors(t *testing.T) {
	hEq(t, plan([]Pkg{{"a", []string{"b"}}, {"b", []string{"x"}}}, "a"), "error: b needs missing x")
	hEq(t, plan([]Pkg{{"a", []string{"b"}}, {"b", []string{"a"}}}, "a"), "error: cycle a -> b -> a")
}
