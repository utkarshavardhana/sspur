package main

import "testing"

func TestRunOk(t *testing.T) {
	if got := run(Add{Num{2}, Mul{Var{"x"}, Num{4}}}, map[string]int{"x": 3}); got != "(2 + x * 4) = 14" {
		t.Fatalf("got %q", got)
	}
}

func TestRunUnbound(t *testing.T) {
	if got := run(Var{"y"}, map[string]int{}); got != "y = error: unbound y" {
		t.Fatalf("got %q", got)
	}
}

func TestSimplifyOk(t *testing.T) {
	if got := show(simplify(Add{Num{0}, Mul{Num{1}, Var{"z"}}})); got != "z" {
		t.Fatalf("got %q", got)
	}
}
