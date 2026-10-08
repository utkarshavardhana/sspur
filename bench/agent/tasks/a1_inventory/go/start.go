package main

import (
	"fmt"
	"strings"
)

type Item struct {
	sku   string
	name  string
	qty   int
	price int
}

// InvErr is implemented by every inventory error.
type InvErr interface {
	error
	invErr()
}

type UnknownSku struct{ sku string }

func (e *UnknownSku) Error() string { return "unknown sku " + e.sku }
func (*UnknownSku) invErr()         {}

type OutOfStock struct {
	sku    string
	wanted int
	have   int
}

func (e *OutOfStock) Error() string {
	return fmt.Sprintf("%s: wanted %d, have %d", e.sku, e.wanted, e.have)
}
func (*OutOfStock) invErr() {}

func sample() []Item {
	return []Item{{"c3", "cable", 7, 12}, {"b2", "bolt", 40, 3}, {"a1", "anchor", 2, 250}}
}

func findItem(inv []Item, sku string) (Item, bool) {
	for _, i := range inv {
		if i.sku == sku {
			return i, true
		}
	}
	return Item{}, false
}

func totalValue(inv []Item) int {
	s := 0
	for _, i := range inv {
		s += i.qty + i.price
	}
	return s
}

func restock(inv []Item, sku string, n int) ([]Item, error) {
	if n <= 0 {
		panic("n must be > 0")
	}
	if _, ok := findItem(inv, sku); !ok {
		return nil, &UnknownSku{sku}
	}
	out := make([]Item, len(inv))
	for k, i := range inv {
		if i.sku == sku {
			i.qty += n
		}
		out[k] = i
	}
	return out, nil
}

func lowStock(inv []Item, threshold int) []string {
	out := []string{}
	for _, i := range inv {
		if i.qty < threshold {
			out = append(out, i.sku)
		}
	}
	return out
}

func describe(i Item) string {
	return fmt.Sprintf("%s %s x%d @ %d", i.sku, i.name, i.qty, i.price)
}

func report(inv []Item) string {
	lines := make([]string, len(inv))
	for k, i := range inv {
		lines[k] = describe(i)
	}
	return strings.Join(lines, "\n")
}

func main() {
	fmt.Println(report(sample()))
}
