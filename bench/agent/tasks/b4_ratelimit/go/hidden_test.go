package main

import (
	"errors"
	"reflect"
	"testing"
)

func hTake(t *testing.T, b Bucket, now, n int) (bool, Bucket) {
	t.Helper()
	ok, b2, err := take(b, now, n)
	if err != nil {
		t.Fatalf("unexpected error %v", err)
	}
	return ok, b2
}

func hDrained(t *testing.T, capacity, rate int) Bucket {
	t.Helper()
	_, b := hTake(t, newBucket(capacity, rate, 0), 0, capacity)
	return b
}

type hStep struct {
	key string
	now int
}

func hKeys(t *testing.T, steps []hStep) []bool {
	t.Helper()
	k := newKeyed(2, 1)
	out := []bool{}
	for _, s := range steps {
		ok, k2, err := takeKey(k, s.key, s.now)
		if err != nil {
			t.Fatalf("unexpected error %v", err)
		}
		k = k2
		out = append(out, ok)
	}
	return out
}

func hTakeKey(t *testing.T, k Keyed, key string, now int) Keyed {
	t.Helper()
	_, k2, err := takeKey(k, key, now)
	if err != nil {
		t.Fatalf("unexpected error %v", err)
	}
	return k2
}

func hRun(t *testing.T, w Window, times []int, want []bool) {
	t.Helper()
	got, err := runRequests(w, times)
	if err != nil {
		t.Fatalf("unexpected error %v", err)
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("got %v, want %v", got, want)
	}
}

func hRetry(t *testing.T, b Bucket, now, n, want int) {
	t.Helper()
	got, err := retryAfter(b, now, n)
	if err != nil || got != want {
		t.Fatalf("retryAfter = %d, %v; want %d", got, err, want)
	}
}

func TestHidden_window_limit(t *testing.T) {
	hRun(t, newWindow(2, 10), []int{0, 1, 2}, []bool{true, true, false})
}

func TestHidden_window_reset(t *testing.T) {
	hRun(t, newWindow(2, 10), []int{0, 1, 2, 10, 11, 12}, []bool{true, true, false, true, true, false})
}

func TestHidden_window_skew(t *testing.T) {
	_, err := runRequests(newWindow(2, 10), []int{10, 3})
	var e *ClockSkew
	if !errors.As(err, &e) || e.last != 10 || e.now != 3 {
		t.Fatalf("err = %v", err)
	}
	var le LimitErr
	if !errors.As(err, &le) {
		t.Fatalf("not a LimitErr: %v", err)
	}
}

func TestHidden_bucket_full(t *testing.T) {
	b := newBucket(5, 1, 100)
	if b.tokens != 5 || b.last != 100 || b.capacity != 5 || b.rate != 1 {
		t.Fatalf("got %+v", b)
	}
}

func TestHidden_take_ok(t *testing.T) {
	ok, b := hTake(t, newBucket(5, 1, 0), 0, 3)
	if !ok || b.tokens != 2 {
		t.Fatalf("ok=%v tokens=%d", ok, b.tokens)
	}
}

func TestHidden_take_short(t *testing.T) {
	_, b1 := hTake(t, newBucket(5, 1, 0), 0, 3)
	ok, b := hTake(t, b1, 0, 3)
	if ok || b.tokens != 2 {
		t.Fatalf("ok=%v tokens=%d", ok, b.tokens)
	}
}

func TestHidden_refill(t *testing.T) {
	ok, b := hTake(t, hDrained(t, 5, 2), 2, 3)
	if !ok || b.tokens != 1 || b.last != 2 {
		t.Fatalf("ok=%v %+v", ok, b)
	}
}

func TestHidden_refill_cap(t *testing.T) {
	if _, b := hTake(t, newBucket(5, 2, 0), 100, 1); b.tokens != 4 {
		t.Fatalf("tokens = %d", b.tokens)
	}
}

func TestHidden_failed_take_refills(t *testing.T) {
	ok, b := hTake(t, hDrained(t, 5, 1), 3, 4)
	if ok || b.tokens != 3 || b.last != 3 {
		t.Fatalf("ok=%v %+v", ok, b)
	}
}

func TestHidden_take_skew(t *testing.T) {
	_, _, err := take(newBucket(5, 1, 10), 9, 1)
	var e *ClockSkew
	if !errors.As(err, &e) || e.last != 10 || e.now != 9 {
		t.Fatalf("err = %v", err)
	}
}

func TestHidden_retry_now(t *testing.T) {
	hRetry(t, newBucket(5, 1, 0), 0, 3, 0)
}

func TestHidden_retry_wait(t *testing.T) {
	hRetry(t, hDrained(t, 5, 2), 0, 3, 2)
	hRetry(t, hDrained(t, 5, 2), 1, 3, 1)
	hRetry(t, hDrained(t, 5, 2), 0, 4, 2)
}

func TestHidden_retry_too_large(t *testing.T) {
	_, err := retryAfter(newBucket(5, 1, 0), 0, 6)
	var e *TooLarge
	if !errors.As(err, &e) || e.n != 6 || e.capacity != 5 {
		t.Fatalf("err = %v", err)
	}
	var le LimitErr
	if !errors.As(err, &le) {
		t.Fatalf("not a LimitErr: %v", err)
	}
}

func TestHidden_retry_skew(t *testing.T) {
	_, err := retryAfter(newBucket(5, 1, 10), 4, 1)
	var e *ClockSkew
	if !errors.As(err, &e) || e.last != 10 || e.now != 4 {
		t.Fatalf("err = %v", err)
	}
}

func TestHidden_keyed(t *testing.T) {
	got := hKeys(t, []hStep{{"x", 0}, {"x", 0}, {"x", 0}, {"y", 0}})
	if !reflect.DeepEqual(got, []bool{true, true, false, true}) {
		t.Fatalf("got %v", got)
	}
}

func TestHidden_keyed_refill(t *testing.T) {
	got := hKeys(t, []hStep{{"x", 0}, {"x", 0}, {"x", 0}, {"x", 3}})
	if !reflect.DeepEqual(got, []bool{true, true, false, true}) {
		t.Fatalf("got %v", got)
	}
}

func TestHidden_keyed_buckets(t *testing.T) {
	if n := len(hTakeKey(t, hTakeKey(t, newKeyed(2, 1), "x", 0), "y", 5).buckets); n != 2 {
		t.Fatalf("buckets = %d", n)
	}
	if k := newKeyed(3, 2); len(k.buckets) != 0 || k.capacity != 3 {
		t.Fatalf("got %+v", k)
	}
}
