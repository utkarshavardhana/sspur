package main

import "testing"

func TestSubtotalSample(t *testing.T) {
	if got := subtotal(sampleOrder()); got != 3150 {
		t.Fatalf("got %d", got)
	}
}

func TestTotalSample(t *testing.T) {
	if got := total(sampleOrder()); got != 3150+499+252 {
		t.Fatalf("got %d", got)
	}
}

func TestInvoiceSample(t *testing.T) {
	want := "order 7: subtotal 31.50, shipping 4.99, tax 2.52, total 39.01"
	if got := invoice(sampleOrder()); got != want {
		t.Fatalf("got %q", got)
	}
}

func TestMoneyPad(t *testing.T) {
	if got := money(1205); got != "12.05" {
		t.Fatalf("got %q", got)
	}
}
