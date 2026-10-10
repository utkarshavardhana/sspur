package orders

import (
	"context"
	"errors"
	"fmt"

	"github.com/shopspring/decimal"
)

type Item struct {
	SKU   string
	Qty   int
	Price decimal.Decimal
}

type Order struct {
	ID    string
	Items []Item
}

var ErrEmpty = errors.New("empty order")

type OutOfStockError struct{ SKU string }

func (e *OutOfStockError) Error() string { return "out of stock: " + e.SKU }

type Store interface {
	Stock(ctx context.Context, sku string) (int, error)
	PutOrder(ctx context.Context, o Order) error
}

func Total(items []Item) decimal.Decimal {
	sum := decimal.Zero
	for _, i := range items {
		sum = sum.Add(i.Price.Mul(decimal.NewFromInt(int64(i.Qty))))
	}
	return sum
}

func Place(ctx context.Context, s Store, o Order) (Order, error) {
	if len(o.Items) == 0 {
		return Order{}, ErrEmpty
	}
	for _, i := range o.Items {
		if i.Qty <= 0 {
			return Order{}, fmt.Errorf("bad qty for %s", i.SKU)
		}
		n, err := s.Stock(ctx, i.SKU)
		if err != nil {
			return Order{}, err
		}
		if n < i.Qty {
			return Order{}, &OutOfStockError{SKU: i.SKU}
		}
	}
	if err := s.PutOrder(ctx, o); err != nil {
		return Order{}, err
	}
	return o, nil
}
