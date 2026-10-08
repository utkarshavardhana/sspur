package main

import (
	"errors"
	"fmt"
	"strings"
)

type Account struct {
	id      string
	owner   string
	balance int
}

// BankErr is implemented by every bank error.
type BankErr interface {
	error
	bankErr()
}

type Insufficient struct {
	id        string
	needed    int
	available int
}

func (e *Insufficient) Error() string {
	return fmt.Sprintf("%s: needed %d, available %d", e.id, e.needed, e.available)
}
func (*Insufficient) bankErr() {}

type SameAccount struct{}

func (*SameAccount) Error() string { return "same account" }
func (*SameAccount) bankErr()      {}

type UnknownAccount struct{ id string }

func (e *UnknownAccount) Error() string { return "unknown account " + e.id }
func (*UnknownAccount) bankErr()        {}

func accounts() []Account {
	return []Account{{"a1", "alice", 100}, {"b1", "bob", 20}, {"c1", "carol", 0}}
}

func openAccount(bank []Account, id string, owner string) []Account {
	return append(append([]Account{}, bank...), Account{id, owner, 0})
}

func lookup(bank []Account, id string) (Account, error) {
	for _, a := range bank {
		if a.id == id {
			return a, nil
		}
	}
	return Account{}, &UnknownAccount{id}
}

func setBalance(bank []Account, id string, b int) []Account {
	if b < 0 {
		panic("balance must be >= 0")
	}
	out := make([]Account, len(bank))
	for k, a := range bank {
		if a.id == id {
			a.balance = b
		}
		out[k] = a
	}
	return out
}

func deposit(bank []Account, id string, amount int) ([]Account, error) {
	if amount <= 0 {
		panic("amount must be > 0")
	}
	a, err := lookup(bank, id)
	if err != nil {
		return nil, err
	}
	return setBalance(bank, id, a.balance+amount), nil
}

func withdraw(bank []Account, id string, amount int) ([]Account, error) {
	if amount <= 0 {
		panic("amount must be > 0")
	}
	a, err := lookup(bank, id)
	if err != nil {
		return nil, err
	}
	if amount > a.balance {
		return nil, &Insufficient{id, amount, a.balance}
	}
	return setBalance(bank, id, a.balance-amount), nil
}

func transfer(bank []Account, frm string, to string, amount int) ([]Account, error) {
	if frm == to {
		return nil, &SameAccount{}
	}
	b, err := withdraw(bank, frm, amount)
	if err != nil {
		return nil, err
	}
	return deposit(b, to, amount)
}

func describe(a Account) string {
	return fmt.Sprintf("%s: %d", a.owner, a.balance)
}

func attempt(bank []Account, frm string, to string, amount int) string {
	b, err := transfer(bank, frm, to, amount)
	var ins *Insufficient
	var same *SameAccount
	var unk *UnknownAccount
	switch {
	case err == nil:
		parts := make([]string, len(b))
		for k, a := range b {
			parts[k] = describe(a)
		}
		return strings.Join(parts, ", ")
	case errors.As(err, &ins):
		return fmt.Sprintf("declined: %s needs %d, has %d", ins.id, ins.needed, ins.available)
	case errors.As(err, &same):
		return "declined: same account"
	case errors.As(err, &unk):
		return "declined: no account " + unk.id
	}
	panic(err)
}

func main() {
	fmt.Println(attempt(accounts(), "a1", "b1", 60))
	fmt.Println(attempt(accounts(), "b1", "a1", 50))
}
