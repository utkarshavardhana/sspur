package main

import "testing"

func hWithCoupon(c string) Order {
	o := sampleOrder()
	o.coupon = c
	return o
}

func hBig(c string, unit int) Order {
	return Order{9, []Line{{"tv", 1, unit}}, c}
}

func hInts(t *testing.T, got []int, want ...int) {
	t.Helper()
	for k := range want {
		if got[k] != want[k] {
			t.Fatalf("got %v, want %v", got, want)
		}
	}
}

func TestHidden_gross(t *testing.T) {
	if got := gross(sampleOrder()); got != 3150 {
		t.Fatalf("got %d", got)
	}
}

func TestHidden_plain_total(t *testing.T) {
	if got := total(sampleOrder()); got != 3901 {
		t.Fatalf("got %d", got)
	}
}

func TestHidden_plain_invoice(t *testing.T) {
	want := "order 7: gross 31.50, discount 0.00, shipping 4.99, tax 2.52, total 39.01"
	if got := invoice(sampleOrder()); got != want {
		t.Fatalf("got %q", got)
	}
}

func TestHidden_save10(t *testing.T) {
	o := hWithCoupon("SAVE10")
	hInts(t, []int{discount(o), tax(o), total(o)}, 315, 226, 3560)
}

func TestHidden_save10_invoice(t *testing.T) {
	want := "order 7: gross 31.50, discount 3.15, shipping 4.99, tax 2.26, total 35.60"
	if got := invoice(hWithCoupon("SAVE10")); got != want {
		t.Fatalf("got %q", got)
	}
}

func TestHidden_big20_small(t *testing.T) {
	if got := discount(hWithCoupon("BIG20")); got != 0 {
		t.Fatalf("got %d", got)
	}
}

func TestHidden_big20(t *testing.T) {
	o := hBig("BIG20", 12000)
	hInts(t, []int{discount(o), shipping(o), total(o)}, 2400, 0, 10368)
}

func TestHidden_net_threshold(t *testing.T) {
	o := hBig("SAVE10", 5400)
	hInts(t, []int{shipping(o), total(o)}, 499, 5747)
}

func TestHidden_gross_threshold(t *testing.T) {
	if got := shipping(hBig("", 5000)); got != 0 {
		t.Fatalf("got %d", got)
	}
}

func TestHidden_freeship(t *testing.T) {
	o := hWithCoupon("FREESHIP")
	hInts(t, []int{shipping(o), discount(o), total(o)}, 0, 0, 3402)
}

func TestHidden_unknown(t *testing.T) {
	if got := total(hWithCoupon("XYZ")); got != 3901 {
		t.Fatalf("got %d", got)
	}
}
