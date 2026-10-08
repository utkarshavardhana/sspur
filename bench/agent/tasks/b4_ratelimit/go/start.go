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

func allow(w Window, now int) (bool, Window) {
	if now >= w.start+w.size {
		w.start = now
		w.count = 0
	}
	if w.count <= w.limit {
		w.count++
		return true, w
	}
	return false, w
}

func runRequests(w Window, times []int) []bool {
	cur := w
	out := []bool{}
	for _, t := range times {
		var ok bool
		ok, cur = allow(cur, t)
		out = append(out, ok)
	}
	return out
}

func main() {
	parts := []string{}
	for _, b := range runRequests(newWindow(3, 10), []int{0, 1, 2, 3, 12}) {
		parts = append(parts, strconv.FormatBool(b))
	}
	fmt.Println(strings.Join(parts, " "))
}
