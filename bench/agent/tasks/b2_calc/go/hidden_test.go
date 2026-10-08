package main

import (
	"errors"
	"strconv"
	"testing"
)

func hRes(s string) string {
	v, err := calc(s)
	if err == nil {
		return strconv.Itoa(v)
	}
	var dz *DivByZero
	var bs *BadSyntax
	var bc *BadChar
	var ce CalcErr
	switch {
	case errors.As(err, &dz):
		return "div"
	case errors.As(err, &bs):
		return "syntax"
	case errors.As(err, &bc):
		return "char " + bc.ch
	case errors.As(err, &ce):
		return "other"
	}
	return "unexpected " + err.Error()
}

func hExpect(t *testing.T, s, want string) {
	t.Helper()
	if got := hRes(s); got != want {
		t.Fatalf("%q: got %q, want %q", s, got, want)
	}
}

func TestHidden_precedence(t *testing.T) {
	hExpect(t, "1 + 2 * 3", "7")
	hExpect(t, "2 * 3 + 4", "10")
	hExpect(t, "20 - 6 / 3", "18")
}

func TestHidden_left_assoc(t *testing.T) {
	hExpect(t, "8 - 3 - 2", "3")
	hExpect(t, "16 / 4 / 2", "2")
	hExpect(t, "2 * 3 / 4", "1")
}

func TestHidden_truncate(t *testing.T) {
	hExpect(t, "7 / 2", "3")
	hExpect(t, "-7 / 2", "-3")
	hExpect(t, "7 / -2", "-3")
}

func TestHidden_no_spaces(t *testing.T) {
	hExpect(t, "12*3-4", "32")
}

func TestHidden_div_zero(t *testing.T) {
	hExpect(t, "1 / 0", "div")
	hExpect(t, "5 / (2 - 2)", "div")
}

func TestHidden_parens(t *testing.T) {
	hExpect(t, "2 * (3 + 4)", "14")
	hExpect(t, "((2))", "2")
	hExpect(t, "(1 + 2) * (3 + 4)", "21")
	hExpect(t, "10 - (2 - 3)", "11")
}

func TestHidden_nested_parens(t *testing.T) {
	hExpect(t, "2 * (3 + (4 - 1) * 2)", "18")
}

func TestHidden_unary(t *testing.T) {
	hExpect(t, "-3", "-3")
	hExpect(t, "2 * -3", "-6")
	hExpect(t, "-(1 + 2)", "-3")
	hExpect(t, "2 - -3", "5")
	hExpect(t, "(-4) * 2", "-8")
}

func TestHidden_empty(t *testing.T) {
	hExpect(t, "", "syntax")
	hExpect(t, "   ", "syntax")
}

func TestHidden_missing_operand(t *testing.T) {
	hExpect(t, "1 +", "syntax")
	hExpect(t, "* 2", "syntax")
}

func TestHidden_two_numbers(t *testing.T) {
	hExpect(t, "1 2", "syntax")
}

func TestHidden_unbalanced(t *testing.T) {
	hExpect(t, "(1 + 2", "syntax")
	hExpect(t, "1 + 2)", "syntax")
}

func TestHidden_empty_parens(t *testing.T) {
	hExpect(t, "()", "syntax")
}

func TestHidden_bad_char(t *testing.T) {
	hExpect(t, "1 $ 2", "char $")
}

func TestHidden_show_div(t *testing.T) {
	if got := showResult("1 / 0"); got != "1 / 0 = error: division by zero" {
		t.Fatalf("got %q", got)
	}
}

func TestHidden_show_syntax(t *testing.T) {
	if got := showResult("1 +"); got != "1 + = error: syntax" {
		t.Fatalf("got %q", got)
	}
}

func TestHidden_show_ok(t *testing.T) {
	if got := showResult("2 * (3 + 4)"); got != "2 * (3 + 4) = 14" {
		t.Fatalf("got %q", got)
	}
	if got := showResult("1 ? 2"); got != "1 ? 2 = error: bad character ?" {
		t.Fatalf("got %q", got)
	}
}
