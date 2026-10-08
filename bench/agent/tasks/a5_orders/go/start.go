package main

import "fmt"

type Line struct {
	sku  string
	qty  int
	unit int
}

type Order struct {
	id     int
	lines  []Line
	coupon string
}

func subtotal(o Order) int {
	s := 0
	for _, l := range o.lines {
		s += l.qty * l.unit
	}
	return s
}

func shipping(o Order) int {
	if subtotal(o) >= 5000 {
		return 0
	}
	return 499
}

func tax(o Order) int {
	return subtotal(o) * 8 / 100
}

func total(o Order) int {
	return subtotal(o) + shipping(o) + tax(o)
}

func money(cents int) string {
	if cents < 0 {
		panic("cents must be >= 0")
	}
	return fmt.Sprintf("%d.%02d", cents/100, cents%100)
}

func invoice(o Order) string {
	return fmt.Sprintf("order %d: subtotal %s, shipping %s, tax %s, total %s",
		o.id, money(subtotal(o)), money(shipping(o)), money(tax(o)), money(total(o)))
}

func sampleOrder() Order {
	return Order{7, []Line{{"pen", 3, 250}, {"pad", 2, 1200}}, ""}
}

func main() {
	fmt.Println(invoice(sampleOrder()))
}
