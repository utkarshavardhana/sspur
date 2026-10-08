package main

import (
	"errors"
	"reflect"
	"testing"
)

func hSkus(inv []Item) []string {
	out := []string{}
	for _, i := range inv {
		out = append(out, i.sku)
	}
	return out
}

func hMustInv(t *testing.T, inv []Item, err error) []Item {
	t.Helper()
	if err != nil {
		t.Fatalf("unexpected error %v", err)
	}
	return inv
}

func TestHidden_total(t *testing.T) {
	if got := totalValue(sample()); got != 7*12+40*3+2*250 {
		t.Fatalf("got %d", got)
	}
}

func TestHidden_total_empty(t *testing.T) {
	if got := totalValue([]Item{}); got != 0 {
		t.Fatalf("got %d", got)
	}
}

func TestHidden_low_sorted(t *testing.T) {
	if got := lowStock(sample(), 10); !reflect.DeepEqual(got, []string{"a1", "c3"}) {
		t.Fatalf("got %v", got)
	}
}

func TestHidden_low_none(t *testing.T) {
	if got := lowStock(sample(), 1); len(got) != 0 {
		t.Fatalf("got %v", got)
	}
}

func TestHidden_remove_ok(t *testing.T) {
	inv, err := removeStock(sample(), "b2", 15)
	if i, _ := findItem(hMustInv(t, inv, err), "b2"); i.qty != 25 {
		t.Fatalf("qty = %d", i.qty)
	}
}

func TestHidden_remove_all(t *testing.T) {
	inv, err := removeStock(sample(), "a1", 2)
	if i, _ := findItem(hMustInv(t, inv, err), "a1"); i.qty != 0 {
		t.Fatalf("qty = %d", i.qty)
	}
}

func TestHidden_remove_keeps_order(t *testing.T) {
	inv, err := removeStock(sample(), "b2", 1)
	if got := hSkus(hMustInv(t, inv, err)); !reflect.DeepEqual(got, []string{"c3", "b2", "a1"}) {
		t.Fatalf("got %v", got)
	}
}

func TestHidden_remove_short(t *testing.T) {
	_, err := removeStock(sample(), "a1", 3)
	var e *OutOfStock
	if !errors.As(err, &e) || e.sku != "a1" || e.wanted != 3 || e.have != 2 {
		t.Fatalf("err = %v", err)
	}
}

func TestHidden_remove_unknown(t *testing.T) {
	_, err := removeStock(sample(), "q9", 1)
	var e *UnknownSku
	if !errors.As(err, &e) || e.sku != "q9" {
		t.Fatalf("err = %v", err)
	}
}

func TestHidden_add_ok(t *testing.T) {
	inv, err := addItem(sample(), Item{"d4", "drill", 1, 900})
	if got := hSkus(hMustInv(t, inv, err)); !reflect.DeepEqual(got, []string{"c3", "b2", "a1", "d4"}) {
		t.Fatalf("got %v", got)
	}
}

func TestHidden_add_dup(t *testing.T) {
	_, err := addItem(sample(), Item{"b2", "bolt", 1, 3})
	var e *DuplicateSku
	if !errors.As(err, &e) || e.sku != "b2" {
		t.Fatalf("err = %v", err)
	}
	var ie InvErr
	if !errors.As(err, &ie) {
		t.Fatalf("not an InvErr: %v", err)
	}
}

func TestHidden_restock_still(t *testing.T) {
	inv, err := restock(sample(), "c3", 3)
	if i, _ := findItem(hMustInv(t, inv, err), "c3"); i.qty != 10 {
		t.Fatalf("qty = %d", i.qty)
	}
}
