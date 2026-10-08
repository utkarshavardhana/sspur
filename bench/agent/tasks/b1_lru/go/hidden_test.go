package main

import (
	"errors"
	"reflect"
	"testing"
)

func hNew(t *testing.T, cap int) Cache {
	t.Helper()
	c, err := newCache(cap)
	if err != nil {
		t.Fatalf("unexpected error %v", err)
	}
	return c
}

func hAbc(t *testing.T, cap int) Cache {
	t.Helper()
	return put(put(put(hNew(t, cap), "a", 1), "b", 2), "c", 3)
}

func hLook(c Cache, key string) Cache {
	_, _, c2 := lookup(c, key)
	return c2
}

func hCounted(t *testing.T) Cache {
	t.Helper()
	return hLook(hLook(hLook(hAbc(t, 3), "a"), "z"), "b")
}

func hResize(t *testing.T, c Cache, cap int) Cache {
	t.Helper()
	c2, err := resize(c, cap)
	if err != nil {
		t.Fatalf("unexpected error %v", err)
	}
	return c2
}

func hKeys(t *testing.T, c Cache, want ...string) {
	t.Helper()
	if got := keys(c); !reflect.DeepEqual(got, want) {
		t.Fatalf("keys = %v, want %v", got, want)
	}
}

func TestHidden_bad_cap_zero(t *testing.T) {
	_, err := newCache(0)
	var e *BadCapacity
	if !errors.As(err, &e) || e.cap != 0 {
		t.Fatalf("err = %v", err)
	}
	var ce CacheErr
	if !errors.As(err, &ce) {
		t.Fatalf("not a CacheErr: %v", err)
	}
}

func TestHidden_bad_cap_neg(t *testing.T) {
	_, err := newCache(-3)
	var e *BadCapacity
	if !errors.As(err, &e) || e.cap != -3 {
		t.Fatalf("err = %v", err)
	}
}

func TestHidden_cap_one(t *testing.T) {
	c := put(put(hNew(t, 1), "a", 1), "b", 2)
	if size(c) != 1 {
		t.Fatalf("size = %d", size(c))
	}
	hKeys(t, c, "b")
}

func TestHidden_evict(t *testing.T) {
	hKeys(t, hAbc(t, 2), "b", "c")
}

func TestHidden_no_evict(t *testing.T) {
	hKeys(t, hAbc(t, 3), "a", "b", "c")
}

func TestHidden_update_existing(t *testing.T) {
	hKeys(t, put(hAbc(t, 3), "a", 9), "b", "c", "a")
	if v, ok := peek(put(hAbc(t, 3), "a", 9), "a"); !ok || v != 9 {
		t.Fatalf("peek = %d %v", v, ok)
	}
}

func TestHidden_update_full(t *testing.T) {
	if n := size(put(hAbc(t, 3), "b", 5)); n != 3 {
		t.Fatalf("size = %d", n)
	}
	hKeys(t, put(hAbc(t, 3), "b", 5), "a", "c", "b")
}

func TestHidden_lookup_recency(t *testing.T) {
	hKeys(t, put(hLook(hAbc(t, 3), "a"), "d", 4), "c", "a", "d")
}

func TestHidden_lookup_value(t *testing.T) {
	if v, ok, _ := lookup(hAbc(t, 3), "b"); !ok || v != 2 {
		t.Fatalf("got %d %v", v, ok)
	}
}

func TestHidden_lookup_miss(t *testing.T) {
	if _, ok, _ := lookup(hAbc(t, 3), "z"); ok {
		t.Fatalf("expected a miss")
	}
	hKeys(t, hLook(hAbc(t, 3), "z"), "a", "b", "c")
}

func TestHidden_counters(t *testing.T) {
	if c := hCounted(t); c.hits != 2 || c.misses != 1 {
		t.Fatalf("hits=%d misses=%d", c.hits, c.misses)
	}
}

func TestHidden_counters_start(t *testing.T) {
	if c := hNew(t, 2); c.hits != 0 || c.misses != 0 {
		t.Fatalf("hits=%d misses=%d", c.hits, c.misses)
	}
}

func TestHidden_get_or_miss(t *testing.T) {
	v, c := getOr(hAbc(t, 3), "z", -1)
	if v != -1 || c.misses != 1 || c.hits != 0 {
		t.Fatalf("v=%d hits=%d misses=%d", v, c.hits, c.misses)
	}
}

func TestHidden_get_or_hit(t *testing.T) {
	v, c := getOr(hAbc(t, 3), "a", -1)
	if v != 1 || c.hits != 1 {
		t.Fatalf("v=%d hits=%d", v, c.hits)
	}
	hKeys(t, c, "b", "c", "a")
}

func TestHidden_peek(t *testing.T) {
	if v, ok := peek(hAbc(t, 3), "a"); !ok || v != 1 {
		t.Fatalf("peek a = %d %v", v, ok)
	}
	if _, ok := peek(hAbc(t, 3), "z"); ok {
		t.Fatalf("peek z found")
	}
	if v, ok := peek(hCounted(t), "a"); !ok || v != 1 {
		t.Fatalf("peek counted a = %d %v", v, ok)
	}
}

func TestHidden_resize_down(t *testing.T) {
	c := hResize(t, hAbc(t, 3), 1)
	hKeys(t, c, "c")
	if c.cap != 1 {
		t.Fatalf("cap = %d", c.cap)
	}
}

func TestHidden_resize_then_put(t *testing.T) {
	hKeys(t, put(hResize(t, hAbc(t, 3), 2), "d", 4), "c", "d")
}

func TestHidden_resize_up(t *testing.T) {
	hKeys(t, put(hResize(t, hAbc(t, 2), 3), "d", 4), "b", "c", "d")
}

func TestHidden_resize_bad(t *testing.T) {
	_, err := resize(hAbc(t, 2), 0)
	var e *BadCapacity
	if !errors.As(err, &e) || e.cap != 0 {
		t.Fatalf("err = %v", err)
	}
}

func TestHidden_put_keeps_counters(t *testing.T) {
	if h := put(hCounted(t), "q", 1).hits; h != 2 {
		t.Fatalf("hits = %d", h)
	}
	if m := hResize(t, hCounted(t), 1).misses; m != 1 {
		t.Fatalf("misses = %d", m)
	}
}

func TestHidden_describe(t *testing.T) {
	if got := describe(hAbc(t, 2)); got != "cap=2 [b=2, c=3]" {
		t.Fatalf("got %q", got)
	}
}
