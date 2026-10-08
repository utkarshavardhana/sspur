package main

import (
	"fmt"
	"strings"
)

type Priority string

const (
	Low  Priority = "low"
	Med  Priority = "med"
	High Priority = "high"
)

type Task struct {
	id    int
	title string
	done  bool
	pri   Priority
}

func nextId(ts []Task) int {
	m := 0
	for _, t := range ts {
		if t.id > m {
			m = t.id
		}
	}
	return m + 1
}

func addTask(ts []Task, title string, pri Priority) []Task {
	return append(append([]Task{}, ts...), Task{nextId(ts), title, false, pri})
}

func complete(ts []Task, id int) []Task {
	out := make([]Task, len(ts))
	for k, t := range ts {
		if t.id == id {
			t.done = true
		}
		out[k] = t
	}
	return out
}

func pending(ts []Task) []Task {
	out := []Task{}
	for _, t := range ts {
		if !t.done {
			out = append(out, t)
		}
	}
	return out
}

func box(t Task) string {
	if t.done {
		return "[x]"
	}
	return "[ ]"
}

func render(t Task) string {
	return fmt.Sprintf("%s #%d %s", box(t), t.id, t.title)
}

func renderAll(ts []Task) string {
	lines := make([]string, len(ts))
	for k, t := range ts {
		lines[k] = render(t)
	}
	return strings.Join(lines, "\n")
}

func demo() []Task {
	ts := []Task{}
	ts = addTask(ts, "buy milk", Low)
	ts = addTask(ts, "write report", High)
	ts = addTask(ts, "call bob", Med)
	ts = addTask(ts, "fix sink", High)
	return ts
}

func main() {
	fmt.Println(renderAll(complete(demo(), 1)))
}
