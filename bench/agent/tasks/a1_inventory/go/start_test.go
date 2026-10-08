package main

import (
	"errors"
	"testing"
)

func TestRestockAdds(t *testing.T) {
	inv, err := restock(sample(), "a1", 3)
	if err != nil {
		t.Fatal(err)
	}
	if i, _ := findItem(inv, "a1"); i.qty != 5 {
		t.Fatalf("qty = %d", i.qty)
	}
}

func TestRestockUnknown(t *testing.T) {
	_, err := restock(sample(), "zz", 1)
	var e *UnknownSku
	if !errors.As(err, &e) || e.sku != "zz" {
		t.Fatalf("err = %v", err)
	}
}

func TestDescribeOne(t *testing.T) {
	if got := describe(Item{"x", "nut", 1, 2}); got != "x nut x1 @ 2" {
		t.Fatalf("got %q", got)
	}
}
