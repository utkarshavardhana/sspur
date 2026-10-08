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

type Bucket struct {
	capacity int
	rate     int
	tokens   int
	last     int
}

type keyBucket struct {
	key    string
	bucket Bucket
}

type Keyed struct {
	capacity int
	rate     int
	buckets  []keyBucket
}

// LimitErr is implemented by every rate limiter error.
type LimitErr interface {
	error
	limitErr()
}

type ClockSkew struct {
	last int
	now  int
}

func (e *ClockSkew) Error() string { return fmt.Sprintf("clock went from %d to %d", e.last, e.now) }
func (*ClockSkew) limitErr()       {}

type TooLarge struct {
	n        int
	capacity int
}

func (e *TooLarge) Error() string { return fmt.Sprintf("%d > capacity %d", e.n, e.capacity) }
func (*TooLarge) limitErr()       {}

func newWindow(limit int, size int) Window {
	return Window{limit, size, 0, 0}
}

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
		ok, next, err := allow(cur, t)
		if err != nil {
			return nil, err
		}
		cur = next
		out = append(out, ok)
	}
	return out, nil
}

func newBucket(capacity int, rate int, now int) Bucket {
	return Bucket{capacity, rate, capacity, now}
}

func refill(b Bucket, now int) (Bucket, error) {
	if now < b.last {
		return b, &ClockSkew{b.last, now}
	}
	b.tokens = min(b.capacity, b.tokens+(now-b.last)*b.rate)
	b.last = now
	return b, nil
}

func take(b Bucket, now int, n int) (bool, Bucket, error) {
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

func retryAfter(b Bucket, now int, n int) (int, error) {
	if n > b.capacity {
		return 0, &TooLarge{n, b.capacity}
	}
	r, err := refill(b, now)
	if err != nil {
		return 0, err
	}
	if r.tokens >= n {
		return 0, nil
	}
	return (n - r.tokens + r.rate - 1) / r.rate, nil
}

func newKeyed(capacity int, rate int) Keyed {
	return Keyed{capacity, rate, []keyBucket{}}
}

func takeKey(k Keyed, key string, now int) (bool, Keyed, error) {
	b := newBucket(k.capacity, k.rate, now)
	for _, kb := range k.buckets {
		if kb.key == key {
			b = kb.bucket
			break
		}
	}
	ok, b2, err := take(b, now, 1)
	if err != nil {
		return false, k, err
	}
	out := []keyBucket{}
	for _, kb := range k.buckets {
		if kb.key != key {
			out = append(out, kb)
		}
	}
	k.buckets = append(out, keyBucket{key, b2})
	return ok, k, nil
}

func main() {
	got, err := runRequests(newWindow(3, 10), []int{0, 1, 2, 3, 12})
	if err != nil {
		panic(err)
	}
	parts := []string{}
	for _, b := range got {
		parts = append(parts, strconv.FormatBool(b))
	}
	fmt.Println(strings.Join(parts, " "))
}
