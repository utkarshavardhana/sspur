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
	return Cache{cap: cap, entries: []Entry{}}, nil
}

func touch(c Cache, key string) (int, bool, Cache) {
	for i, e := range c.entries {
		if e.key == key {
			out := make([]Entry, 0, len(c.entries))
			out = append(out, c.entries[:i]...)
			out = append(out, c.entries[i+1:]...)
			c.entries = append(out, e)
			return e.value, true, c
		}
	}
	return 0, false, c
}

func lookup(c Cache, key string) (int, bool, Cache) {
	v, ok, c := touch(c, key)
	if ok {
		c.hits++
	} else {
		c.misses++
	}
	return v, ok, c
}

func getOr(c Cache, key string, def int) (int, Cache) {
	v, ok, c := lookup(c, key)
	if ok {
		return v, c
	}
	return def, c
}

func peek(c Cache, key string) (int, bool) {
	for _, e := range c.entries {
		if e.key == key {
			return e.value, true
		}
	}
	return 0, false
}

func evictTo(c Cache, n int) Cache {
	if len(c.entries) > n {
		c.entries = append([]Entry{}, c.entries[len(c.entries)-n:]...)
	}
	return c
}

func put(c Cache, key string, value int) Cache {
	out := []Entry{}
	for _, e := range c.entries {
		if e.key != key {
			out = append(out, e)
		}
	}
	c.entries = out
	if c.cap >= 1 {
		c = evictTo(c, c.cap-1)
	}
	c.entries = append(c.entries, Entry{key, value})
	return c
}

func resize(c Cache, cap int) (Cache, error) {
	if cap < 1 {
		return c, &BadCapacity{cap}
	}
	c.cap = cap
	return evictTo(c, cap), nil
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
