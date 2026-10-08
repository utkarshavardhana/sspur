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

func gross(o Order) int {
	s := 0
	for _, l := range o.lines {
		s += l.qty * l.unit
	}
	return s
}

func discount(o Order) int {
	g := gross(o)
	switch o.coupon {
	case "SAVE10":
		return g * 10 / 100
	case "BIG20":
		if g >= 10000 {
			return g * 20 / 100
		}
	}
	return 0
}

func net(o Order) int {
	return gross(o) - discount(o)
}

func shipping(o Order) int {
	if net(o) >= 5000 || o.coupon == "FREESHIP" {
		return 0
	}
	return 499
}

func tax(o Order) int {
	return net(o) * 8 / 100
}

func total(o Order) int {
	return net(o) + shipping(o) + tax(o)
}

func money(cents int) string {
	if cents < 0 {
		panic("cents must be >= 0")
	}
	return fmt.Sprintf("%d.%02d", cents/100, cents%100)
}

func invoice(o Order) string {
	return fmt.Sprintf("order %d: gross %s, discount %s, shipping %s, tax %s, total %s",
		o.id, money(gross(o)), money(discount(o)), money(shipping(o)), money(tax(o)), money(total(o)))
}

func sampleOrder() Order {
	return Order{7, []Line{{"pen", 3, 250}, {"pad", 2, 1200}}, ""}
}

func main() {
	fmt.Println(invoice(sampleOrder()))
}
