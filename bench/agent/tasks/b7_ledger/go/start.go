package main

import (
	"fmt"
	"slices"
)

type Posting struct {
	account string
	amount  int
}

type Txn struct {
	id       int
	memo     string
	postings []Posting
}

type Ledger struct {
	accounts []string
	txns     []Txn
}

// AccountBalance is one row of the trial balance.
type AccountBalance struct {
	account string
	balance int
}

// LedgerErr is implemented by every ledger error.
type LedgerErr interface {
	error
	ledgerErr()
}

type UnknownAccount struct{ name string }

func (e *UnknownAccount) Error() string { return "unknown account " + e.name }
func (*UnknownAccount) ledgerErr()      {}

func chart() []string {
	return []string{"income:salary", "assets:bank", "expenses:food", "assets:cash", "equity:owner"}
}

func newLedger() Ledger {
	return Ledger{chart(), []Txn{}}
}

func p(account string, amount int) Posting {
	return Posting{account, amount}
}

func addTxn(l Ledger, t Txn) (Ledger, error) {
	for _, x := range t.postings {
		if !slices.Contains(l.accounts, x.account) {
			return l, &UnknownAccount{x.account}
		}
	}
	l.txns = append(append([]Txn{}, l.txns...), t)
	return l, nil
}

func balance(l Ledger, account string) int {
	s := 0
	for _, t := range l.txns {
		for _, x := range t.postings {
			if x.account == account {
				s += x.amount
			}
		}
	}
	return s
}

func trialBalance(l Ledger) []AccountBalance {
	out := []AccountBalance{}
	for _, a := range l.accounts {
		out = append(out, AccountBalance{a, balance(l, a)})
	}
	return out
}

func sample() Ledger {
	l, err := addTxn(newLedger(), Txn{1, "open", []Posting{p("assets:bank", 1000), p("equity:owner", -1000)}})
	if err != nil {
		panic(err)
	}
	l, err = addTxn(l, Txn{2, "lunch", []Posting{p("expenses:food", 30), p("assets:bank", -30)}})
	if err != nil {
		panic(err)
	}
	return l
}

func main() {
	for _, ab := range trialBalance(sample()) {
		fmt.Printf("%s %d\n", ab.account, ab.balance)
	}
}
