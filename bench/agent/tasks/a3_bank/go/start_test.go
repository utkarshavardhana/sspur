package main

import "testing"

func TestBigTransfer(t *testing.T) {
	if got := attempt(accounts(), "a1", "b1", 60); got != "alice: 40, bob: 80, carol: 0" {
		t.Fatalf("got %q", got)
	}
}

func TestDeclined(t *testing.T) {
	if got := attempt(accounts(), "b1", "a1", 50); got != "declined: b1 needs 50, has 20" {
		t.Fatalf("got %q", got)
	}
}

func TestUnknown(t *testing.T) {
	if got := attempt(accounts(), "a1", "zz", 5); got != "declined: no account zz" {
		t.Fatalf("got %q", got)
	}
}

func TestOpened(t *testing.T) {
	bank := openAccount(accounts(), "d1", "dan")
	if got := describe(bank[len(bank)-1]); got != "dan: 0" {
		t.Fatalf("got %q", got)
	}
}
