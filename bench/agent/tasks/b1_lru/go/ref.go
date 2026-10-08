package main

import (
	"fmt"
	"strings"
)

type Entry struct {
	key   string
	value int
}

type Cache struct {
	cap     int
	entries []Entry
	hits    int
	misses  int
}

// CacheErr is implemented by every cache error.
type CacheErr interface {
	error
	cacheErr()
}

type BadCapacity struct{ cap int }

func (e *BadCapacity) Error() string { return fmt.Sprintf("bad capacity %d", e.cap) }
func (*BadCapacity) cacheErr()       {}

func newCache(cap int) (Cache, error) {
	if cap < 1 {
		return Cache{}, &BadCapacity{cap}
	}
	return Cache{cap, []Entry{}, 0, 0}, nil
}

func trim(c Cache) Cache {
	if n := len(c.entries) - c.cap; n > 0 {
		c.entries = append([]Entry{}, c.entries[n:]...)
	}
	return c
}

func touch(c Cache, key string, value int) Cache {
	out := []Entry{}
	for _, e := range c.entries {
		if e.key != key {
			out = append(out, e)
		}
	}
	c.entries = append(out, Entry{key, value})
	return c
}

func peek(c Cache, key string) (int, bool) {
	for _, e := range c.entries {
		if e.key == key {
			return e.value, true
		}
	}
	return 0, false
}

func lookup(c Cache, key string) (int, bool, Cache) {
	v, ok := peek(c, key)
	if !ok {
		c.misses++
		return 0, false, c
	}
	c = touch(c, key, v)
	c.hits++
	return v, true, c
}

func getOr(c Cache, key string, def int) (int, Cache) {
	v, ok, c2 := lookup(c, key)
	if !ok {
		return def, c2
	}
	return v, c2
}

func put(c Cache, key string, value int) Cache {
	return trim(touch(c, key, value))
}

func resize(c Cache, cap int) (Cache, error) {
	if cap < 1 {
		return Cache{}, &BadCapacity{cap}
	}
	c.cap = cap
	return trim(c), nil
}

func keys(c Cache) []string {
	out := []string{}
	for _, e := range c.entries {
		out = append(out, e.key)
	}
	return out
}

func size(c Cache) int {
	return len(c.entries)
}

func describe(c Cache) string {
	parts := []string{}
	for _, e := range c.entries {
		parts = append(parts, fmt.Sprintf("%s=%d", e.key, e.value))
	}
	return fmt.Sprintf("cap=%d [", c.cap) + strings.Join(parts, ", ") + "]"
}

func main() {
	c, _ := newCache(2)
	fmt.Println(describe(put(put(c, "a", 1), "b", 2)))
}
