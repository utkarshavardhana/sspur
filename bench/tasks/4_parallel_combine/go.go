package summary

import (
	"context"

	"golang.org/x/sync/errgroup"
)

type Summary struct {
	Name      string
	ItemCount int
	Total     int64
}

func Build(ctx context.Context, api API, userID string) (Summary, error) {
	var user User
	var cart Cart
	g, ctx := errgroup.WithContext(ctx)
	g.Go(func() (err error) {
		user, err = api.GetUser(ctx, userID)
		return err
	})
	g.Go(func() (err error) {
		cart, err = api.GetCart(ctx, userID)
		return err
	})
	if err := g.Wait(); err != nil {
		return Summary{}, err
	}
	var total int64
	for _, i := range cart.Items {
		total += i.PriceCents * int64(i.Qty)
	}
	return Summary{Name: user.Name, ItemCount: len(cart.Items), Total: total}, nil
}
