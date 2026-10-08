package main

import (
	"errors"
	"reflect"
	"testing"
)

func hMustBank(t *testing.T, bank []Account, err error) []Account {
	t.Helper()
	if err != nil {
		t.Fatalf("unexpected error %v", err)
	}
	return bank
}

func hFrozen(t *testing.T, id string) []Account {
	t.Helper()
	bank, err := freeze(accounts(), id)
	return hMustBank(t, bank, err)
}

func hExpect(t *testing.T, got, want string) {
	t.Helper()
	if got != want {
		t.Fatalf("got %q, want %q", got, want)
	}
}

func TestHidden_no_fee(t *testing.T) {
	hExpect(t, attempt(accounts(), "a1", "b1", 60), "alice: 40, bob: 80, carol: 0")
}

func TestHidden_fee(t *testing.T) {
	hExpect(t, attempt(accounts(), "a1", "b1", 10), "alice: 89, bob: 30, carol: 0")
}

func TestHidden_fee_short(t *testing.T) {
	hExpect(t, attempt(accounts(), "b1", "a1", 20), "declined: b1 needs 21, has 20")
}

func TestHidden_fee_exact(t *testing.T) {
	hExpect(t, attempt(accounts(), "b1", "a1", 19), "alice: 119, bob: 0, carol: 0")
}

func TestHidden_frozen_to(t *testing.T) {
	hExpect(t, attempt(hFrozen(t, "b1"), "a1", "b1", 60), "declined: b1 is frozen")
}

func TestHidden_frozen_from(t *testing.T) {
	hExpect(t, attempt(hFrozen(t, "a1"), "a1", "b1", 60), "declined: a1 is frozen")
}

func TestHidden_frozen_other(t *testing.T) {
	hExpect(t, attempt(hFrozen(t, "c1"), "a1", "b1", 60), "alice: 40, bob: 80, carol: 0")
}

func TestHidden_deposit_frozen(t *testing.T) {
	_, err := deposit(hFrozen(t, "c1"), "c1", 5)
	var e *Frozen
	if !errors.As(err, &e) || e.id != "c1" {
		t.Fatalf("err = %v", err)
	}
	var be BankErr
	if !errors.As(err, &be) {
		t.Fatalf("not a BankErr: %v", err)
	}
}

func TestHidden_withdraw_frozen(t *testing.T) {
	_, err := withdraw([]Account{{"z", "zed", 5, true}}, "z", 1)
	var e *Frozen
	if !errors.As(err, &e) || e.id != "z" {
		t.Fatalf("err = %v", err)
	}
}

func TestHidden_freeze_unknown(t *testing.T) {
	_, err := freeze(accounts(), "q")
	var e *UnknownAccount
	if !errors.As(err, &e) || e.id != "q" {
		t.Fatalf("err = %v", err)
	}
}

func TestHidden_freeze_sets(t *testing.T) {
	got := []bool{}
	for _, a := range hFrozen(t, "b1") {
		got = append(got, a.frozen)
	}
	if !reflect.DeepEqual(got, []bool{false, true, false}) {
		t.Fatalf("got %v", got)
	}
}

func TestHidden_unfrozen(t *testing.T) {
	for _, a := range accounts() {
		if a.frozen {
			t.Fatalf("%s starts frozen", a.id)
		}
	}
	bank := openAccount(accounts(), "d1", "dan")
	if bank[len(bank)-1].frozen {
		t.Fatalf("opened account starts frozen")
	}
}

func TestHidden_same(t *testing.T) {
	hExpect(t, attempt(accounts(), "a1", "a1", 5), "declined: same account")
}
