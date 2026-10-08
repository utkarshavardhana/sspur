package main

import (
	"fmt"
	"slices"
	"strings"
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

type Unbalanced struct{ id, diff int }

func (e *Unbalanced) Error() string { return fmt.Sprintf("txn %d unbalanced by %d", e.id, e.diff) }
func (*Unbalanced) ledgerErr()      {}

type TooFewPostings struct{ id int }

func (e *TooFewPostings) Error() string { return fmt.Sprintf("txn %d has too few postings", e.id) }
func (*TooFewPostings) ledgerErr()      {}

type DuplicateId struct{ id int }

func (e *DuplicateId) Error() string { return fmt.Sprintf("duplicate txn id %d", e.id) }
func (*DuplicateId) ledgerErr()      {}

type Overdrawn struct {
	account string
	balance int
}

func (e *Overdrawn) Error() string { return fmt.Sprintf("%s overdrawn: %d", e.account, e.balance) }
func (*Overdrawn) ledgerErr()      {}

type UnknownTxn struct{ id int }

func (e *UnknownTxn) Error() string { return fmt.Sprintf("unknown txn %d", e.id) }
func (*UnknownTxn) ledgerErr()      {}

type HistEntry struct{ id, balance int }

func addTxn(l Ledger, t Txn) (Ledger, error) {
	if len(t.postings) < 2 {
		return l, &TooFewPostings{t.id}
	}
	sum := 0
	for _, x := range t.postings {
		if !slices.Contains(l.accounts, x.account) {
			return l, &UnknownAccount{x.account}
		}
		sum += x.amount
	}
	if sum != 0 {
		return l, &Unbalanced{t.id, sum}
	}
	for _, o := range l.txns {
		if o.id == t.id {
			return l, &DuplicateId{t.id}
		}
	}
	for _, x := range t.postings {
		if !strings.HasPrefix(x.account, "assets:") {
			continue
		}
		b := balance(l, x.account)
		for _, y := range t.postings {
			if y.account == x.account {
				b += y.amount
			}
		}
		if b < 0 {
			return l, &Overdrawn{x.account, b}
		}
	}
	l.txns = append(append([]Txn{}, l.txns...), t)
	return l, nil
}

func reverse(l Ledger, id int, newId int) (Ledger, error) {
	for _, o := range l.txns {
		if o.id == id {
			ps := make([]Posting, len(o.postings))
			for i, x := range o.postings {
				ps[i] = Posting{x.account, -x.amount}
			}
			return addTxn(l, Txn{newId, fmt.Sprintf("reverse %d", id), ps})
		}
	}
	return l, &UnknownTxn{id}
}

func history(l Ledger, account string) []HistEntry {
	out := []HistEntry{}
	s := 0
	for _, t := range l.txns {
		touched := false
		for _, x := range t.postings {
			if x.account == account {
				s += x.amount
				touched = true
			}
		}
		if touched {
			out = append(out, HistEntry{t.id, s})
		}
	}
	return out
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
		if b := balance(l, a); b != 0 {
			out = append(out, AccountBalance{a, b})
		}
	}
	slices.SortFunc(out, func(x, y AccountBalance) int { return strings.Compare(x.account, y.account) })
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
