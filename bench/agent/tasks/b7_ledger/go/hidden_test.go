package main

import (
	"errors"
	"fmt"
	"reflect"
	"testing"
)

func hAdd(l Ledger, id int, ps []Posting) string {
	l2, err := addTxn(l, Txn{id, "t", ps})
	if err == nil {
		return fmt.Sprintf("ok %d", len(l2.txns))
	}
	var ua *UnknownAccount
	var ub *Unbalanced
	var tf *TooFewPostings
	var di *DuplicateId
	var od *Overdrawn
	var le LedgerErr
	switch {
	case errors.As(err, &ua):
		return "unknown " + ua.name
	case errors.As(err, &ub):
		return fmt.Sprintf("unbalanced %d %d", ub.id, ub.diff)
	case errors.As(err, &tf):
		return fmt.Sprintf("few %d", tf.id)
	case errors.As(err, &di):
		return fmt.Sprintf("dup %d", di.id)
	case errors.As(err, &od):
		return fmt.Sprintf("overdrawn %s %d", od.account, od.balance)
	case errors.As(err, &le):
		return "other"
	}
	return "unexpected " + err.Error()
}

func hSampleAdd(id int, ps []Posting) string {
	return hAdd(sample(), id, ps)
}

func hRev(id, newId int) string {
	l, err := reverse(sample(), id, newId)
	if err == nil {
		return fmt.Sprintf("ok %d", balance(l, "assets:bank"))
	}
	var ut *UnknownTxn
	var di *DuplicateId
	var od *Overdrawn
	var le LedgerErr
	switch {
	case errors.As(err, &ut):
		return fmt.Sprintf("no txn %d", ut.id)
	case errors.As(err, &di):
		return fmt.Sprintf("dup %d", di.id)
	case errors.As(err, &od):
		return fmt.Sprintf("overdrawn %s %d", od.account, od.balance)
	case errors.As(err, &le):
		return "other"
	}
	return "unexpected " + err.Error()
}

type hRow struct {
	account string
	balance int
}

func hTrial(l Ledger) []hRow {
	out := []hRow{}
	for _, x := range trialBalance(l) {
		out = append(out, hRow{x.account, x.balance})
	}
	return out
}

func hHist(l Ledger, account string) [][2]int {
	out := [][2]int{}
	for _, x := range history(l, account) {
		out = append(out, [2]int{x.id, x.balance})
	}
	return out
}

func hEq(t *testing.T, got, want any) {
	t.Helper()
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("got %v, want %v", got, want)
	}
}

func TestHidden_sample(t *testing.T) {
	hEq(t, balance(sample(), "assets:bank"), 970)
	hEq(t, len(sample().txns), 2)
}

func TestHidden_unbalanced(t *testing.T) {
	hEq(t, hAdd(newLedger(), 1, []Posting{p("assets:bank", 10), p("equity:owner", -9)}), "unbalanced 1 1")
	hEq(t, hAdd(newLedger(), 4, []Posting{p("assets:bank", 5), p("equity:owner", -8)}), "unbalanced 4 -3")
}

func TestHidden_too_few(t *testing.T) {
	hEq(t, hAdd(newLedger(), 1, []Posting{p("assets:bank", 0)}), "few 1")
	hEq(t, hAdd(newLedger(), 2, []Posting{}), "few 2")
}

func TestHidden_unknown(t *testing.T) {
	hEq(t, hAdd(newLedger(), 1, []Posting{p("assets:nope", 5), p("equity:owner", -5)}), "unknown assets:nope")
}

func TestHidden_check_order(t *testing.T) {
	hEq(t, hAdd(newLedger(), 1, []Posting{p("zzz", 5)}), "few 1")
	hEq(t, hAdd(newLedger(), 1, []Posting{p("zzz", 5), p("equity:owner", -1)}), "unknown zzz")
}

func TestHidden_duplicate(t *testing.T) {
	hEq(t, hSampleAdd(2, []Posting{p("expenses:food", 5), p("assets:bank", -5)}), "dup 2")
}

func TestHidden_overdrawn(t *testing.T) {
	hEq(t, hSampleAdd(3, []Posting{p("expenses:food", 2000), p("assets:bank", -2000)}), "overdrawn assets:bank -1030")
}

func TestHidden_overdrawn_cash(t *testing.T) {
	hEq(t, hSampleAdd(3, []Posting{p("assets:cash", -1), p("assets:bank", 1)}), "overdrawn assets:cash -1")
}

func TestHidden_exact_zero(t *testing.T) {
	hEq(t, hSampleAdd(3, []Posting{p("expenses:food", 970), p("assets:bank", -970)}), "ok 3")
}

func TestHidden_other_negative(t *testing.T) {
	hEq(t, hSampleAdd(3, []Posting{p("assets:cash", 50), p("income:salary", -50)}), "ok 3")
}

func TestHidden_trial(t *testing.T) {
	hEq(t, hTrial(sample()), []hRow{{"assets:bank", 970}, {"equity:owner", -1000}, {"expenses:food", 30}})
}

func TestHidden_trial_empty(t *testing.T) {
	hEq(t, hTrial(newLedger()), []hRow{})
}

func TestHidden_reverse(t *testing.T) {
	hEq(t, hRev(2, 3), "ok 1000")
}

func TestHidden_reverse_txn(t *testing.T) {
	l, err := reverse(sample(), 2, 3)
	if err != nil {
		t.Fatalf("unexpected error %v", err)
	}
	hEq(t, l.txns[len(l.txns)-1], Txn{3, "reverse 2", []Posting{p("expenses:food", -30), p("assets:bank", 30)}})
	hEq(t, hTrial(l), []hRow{{"assets:bank", 1000}, {"equity:owner", -1000}})
}

func TestHidden_reverse_errors(t *testing.T) {
	hEq(t, hRev(9, 3), "no txn 9")
	hEq(t, hRev(2, 1), "dup 1")
	hEq(t, hRev(1, 3), "overdrawn assets:bank -30")
}

func TestHidden_history(t *testing.T) {
	hEq(t, hHist(sample(), "assets:bank"), [][2]int{{1, 1000}, {2, 970}})
	hEq(t, hHist(sample(), "expenses:food"), [][2]int{{2, 30}})
	hEq(t, hHist(sample(), "assets:cash"), [][2]int{})
}

func TestHidden_history_net(t *testing.T) {
	l, err := addTxn(sample(), Txn{7, "x", []Posting{p("assets:bank", 5), p("assets:bank", 5), p("equity:owner", -10)}})
	if err != nil {
		t.Fatalf("unexpected error %v", err)
	}
	hEq(t, hHist(l, "assets:bank"), [][2]int{{1, 1000}, {2, 970}, {7, 980}})
}
