package main

import (
	"errors"
	"reflect"
	"testing"
)

func hExpect(t *testing.T, got, want string) {
	t.Helper()
	if got != want {
		t.Fatalf("got %q, want %q", got, want)
	}
}

func TestHidden_sub(t *testing.T) {
	hExpect(t, run(Sub{Num{10}, Var{"x"}}, map[string]int{"x": 3}), "(10 - x) = 7")
}

func TestHidden_div(t *testing.T) {
	hExpect(t, run(Div{Num{7}, Num{2}}, map[string]int{}), "7 / 2 = 3")
}

func TestHidden_div_zero(t *testing.T) {
	hExpect(t, run(Div{Num{1}, Sub{Var{"x"}, Var{"x"}}}, map[string]int{"x": 4}), "1 / (x - x) = error: division by zero")
}

func TestHidden_unbound_still(t *testing.T) {
	hExpect(t, run(Sub{Var{"w"}, Num{1}}, map[string]int{}), "(w - 1) = error: unbound w")
}

func TestHidden_eval_neg(t *testing.T) {
	v, err := evalExpr(Mul{Sub{Num{2}, Num{5}}, Num{3}}, map[string]int{})
	if err != nil || v != -9 {
		t.Fatalf("got %d, %v", v, err)
	}
}

func TestHidden_eval_divzero(t *testing.T) {
	_, err := evalExpr(Div{Num{5}, Num{0}}, map[string]int{})
	var e *DivByZero
	if !errors.As(err, &e) {
		t.Fatalf("err = %v", err)
	}
	var ce CalcErr
	if !errors.As(err, &ce) {
		t.Fatalf("not a CalcErr: %v", err)
	}
}

func TestHidden_simp_sub(t *testing.T) {
	hExpect(t, show(simplify(Sub{Var{"y"}, Num{0}})), "y")
}

func TestHidden_simp_div(t *testing.T) {
	hExpect(t, show(simplify(Div{Add{Num{0}, Var{"y"}}, Num{1}})), "y")
}

func TestHidden_simp_inner(t *testing.T) {
	hExpect(t, show(simplify(Sub{Mul{Num{1}, Var{"a"}}, Var{"b"}})), "(a - b)")
}

func TestHidden_simp_div_inner(t *testing.T) {
	hExpect(t, show(simplify(Div{Var{"a"}, Add{Num{2}, Num{0}}})), "a / 2")
}

func TestHidden_vars(t *testing.T) {
	got := varNames(Add{Var{"x"}, Mul{Var{"y"}, Div{Var{"x"}, Var{"z"}}}})
	if !reflect.DeepEqual(got, []string{"x", "y", "z"}) {
		t.Fatalf("got %v", got)
	}
}

func TestHidden_vars_none(t *testing.T) {
	if got := varNames(Sub{Num{3}, Num{1}}); len(got) != 0 {
		t.Fatalf("got %v", got)
	}
}

func TestHidden_old(t *testing.T) {
	hExpect(t, run(Add{Num{2}, Mul{Var{"x"}, Num{4}}}, map[string]int{"x": 3}), "(2 + x * 4) = 14")
}
