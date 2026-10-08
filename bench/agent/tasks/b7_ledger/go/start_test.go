package main

import (
	"errors"
	"testing"
)

func TestBank(t *testing.T) {
	if got := balance(sample(), "assets:bank"); got != 970 {
		t.Fatalf("got %d", got)
	}
}

func TestUnknown(t *testing.T) {
	_, err := addTxn(newLedger(), Txn{1, "x", []Posting{p("assets:gold", 5), p("equity:owner", -5)}})
	var e *UnknownAccount
	if !errors.As(err, &e) || e.name != "assets:gold" {
		t.Fatalf("err = %v", err)
	}
}

func TestTrialSumsToZero(t *testing.T) {
	s := 0
	for _, ab := range trialBalance(sample()) {
		s += ab.balance
	}
	if s != 0 {
		t.Fatalf("sum = %d", s)
	}
}
