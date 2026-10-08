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
}

// CacheErr is implemented by every cache error.
type CacheErr interface {
	error
	cacheErr()
}

type BadCapacity struct{ cap int }

func (e *BadCapacity) Error() string { return fmt.Sprintf("bad capacity %d", e.cap) }
func (*BadCapacity) cacheErr()       {}

func newCache(cap int) Cache {
	return Cache{cap, []Entry{}}
}

func lookup(c Cache, key string) (int, bool) {
	for _, e := range c.entries {
		if e.key == key {
			return e.value, true
		}
	}
	return 0, false
}

func getOr(c Cache, key string, def int) int {
	if v, ok := lookup(c, key); ok {
		return v
	}
	return def
}

func put(c Cache, key string, value int) Cache {
	out := []Entry{}
	for _, e := range c.entries {
		if e.key != key {
			out = append(out, e)
		}
	}
	c.entries = append(out, Entry{key, value})
	return c
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
	fmt.Println(describe(put(put(newCache(2), "a", 1), "b", 2)))
}
