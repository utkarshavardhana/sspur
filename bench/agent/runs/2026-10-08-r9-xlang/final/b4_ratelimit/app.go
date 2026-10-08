package main

import (
	"fmt"
	"strconv"
	"strings"
)

type Window struct {
	limit int
	size  int
	start int
	count int
}

func newWindow(limit int, size int) Window {
	return Window{limit, size, 0, 0}
}

type LimitErr interface {
	error
	limitErr()
}

type ClockSkew struct{ last, now int }

func (e *ClockSkew) Error() string {
	return fmt.Sprintf("clock skew: now %d before last %d", e.now, e.last)
}
func (e *ClockSkew) limitErr() {}

type TooLarge struct{ n, capacity int }

func (e *TooLarge) Error() string {
	return fmt.Sprintf("request %d exceeds capacity %d", e.n, e.capacity)
}
func (e *TooLarge) limitErr() {}

func allow(w Window, now int) (bool, Window, error) {
	if now < w.start {
		return false, w, &ClockSkew{w.start, now}
	}
	if now >= w.start+w.size {
		w.start = now
		w.count = 0
	}
	if w.count < w.limit {
		w.count++
		return true, w, nil
	}
	return false, w, nil
}

func runRequests(w Window, times []int) ([]bool, error) {
	cur := w
	out := []bool{}
	for _, t := range times {
		var ok bool
		var err error
		ok, cur, err = allow(cur, t)
		if err != nil {
			return nil, err
		}
		out = append(out, ok)
	}
	return out, nil
}

type Bucket struct{ capacity, rate, tokens, last int }

func newBucket(capacity, rate, now int) Bucket {
	return Bucket{capacity, rate, capacity, now}
}

func refill(b Bucket, now int) (Bucket, error) {
	if now < b.last {
		return b, &ClockSkew{b.last, now}
	}
	t := b.tokens + (now-b.last)*b.rate
	if t > b.capacity {
		t = b.capacity
	}
	b.tokens = t
	b.last = now
	return b, nil
}

func take(b Bucket, now, n int) (bool, Bucket, error) {
	r, err := refill(b, now)
	if err != nil {
		return false, b, err
	}
	if r.tokens >= n {
		r.tokens -= n
		return true, r, nil
	}
	return false, r, nil
}

func retryAfter(b Bucket, now, n int) (int, error) {
	if n > b.capacity {
		return 0, &TooLarge{n, b.capacity}
	}
	r, err := refill(b, now)
	if err != nil {
		return 0, err
	}
	need := n - r.tokens
	if need <= 0 {
		return 0, nil
	}
	if r.rate <= 0 {
		return 0, &TooLarge{n, b.capacity}
	}
	return (need + r.rate - 1) / r.rate, nil
}

type P struct {
	key    string
	bucket Bucket
}

type Keyed struct {
	capacity, rate int
	buckets        []P
}

func newKeyed(capacity, rate int) Keyed {
	return Keyed{capacity: capacity, rate: rate}
}

func takeKey(k Keyed, key string, now int) (bool, Keyed, error) {
	nb := make([]P, len(k.buckets), len(k.buckets)+1)
	copy(nb, k.buckets)
	k.buckets = nb
	for i, p := range k.buckets {
		if p.key == key {
			ok, b, err := take(p.bucket, now, 1)
			if err != nil {
				return false, k, err
			}
			k.buckets[i].bucket = b
			return ok, k, nil
		}
	}
	ok, b, err := take(newBucket(k.capacity, k.rate, now), now, 1)
	if err != nil {
		return false, k, err
	}
	k.buckets = append(k.buckets, P{key, b})
	return ok, k, nil
}

func main() {
	parts := []string{}
	res, _ := runRequests(newWindow(3, 10), []int{0, 1, 2, 3, 12})
	for _, b := range res {
		parts = append(parts, strconv.FormatBool(b))
	}
	fmt.Println(strings.Join(parts, " "))
}
