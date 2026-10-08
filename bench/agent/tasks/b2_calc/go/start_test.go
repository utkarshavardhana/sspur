package main

import (
	"errors"
	"testing"
)

func TestAdds(t *testing.T) {
	if v, err := calc("1 + 2 + 3"); err != nil || v != 6 {
		t.Fatalf("got %d, %v", v, err)
	}
}

func TestSubtracts(t *testing.T) {
	if v, err := calc("10 - 4 - 3"); err != nil || v != 3 {
		t.Fatalf("got %d, %v", v, err)
	}
}

func TestBadChar(t *testing.T) {
	_, err := calc("2 $ 3")
	var e *BadChar
	if !errors.As(err, &e) || e.ch != "$" {
		t.Fatalf("err = %v", err)
	}
}

func TestShown(t *testing.T) {
	if got := showResult("7 - 2"); got != "7 - 2 = 5" {
		t.Fatalf("got %q", got)
	}
}
