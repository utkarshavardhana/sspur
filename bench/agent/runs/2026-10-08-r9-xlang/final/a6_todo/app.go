package main

import (
	"fmt"
	"sort"
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

type TaskErr interface {
	error
	taskErr()
}

type NotFound struct{ id int }

type AlreadyDone struct{ id int }

func (e *NotFound) Error() string    { return fmt.Sprintf("task %d not found", e.id) }
func (e *NotFound) taskErr()         {}
func (e *AlreadyDone) Error() string { return fmt.Sprintf("task %d already done", e.id) }
func (e *AlreadyDone) taskErr()      {}

func complete(ts []Task, id int) ([]Task, error) {
	out := make([]Task, len(ts))
	found := false
	for k, t := range ts {
		if t.id == id {
			if t.done {
				return ts, &AlreadyDone{id}
			}
			t.done = true
			found = true
		}
		out[k] = t
	}
	if !found {
		return ts, &NotFound{id}
	}
	return out, nil
}

func retitle(ts []Task, id int, title string) ([]Task, error) {
	out := make([]Task, len(ts))
	found := false
	for k, t := range ts {
		if t.id == id {
			t.title = title
			found = true
		}
		out[k] = t
	}
	if !found {
		return ts, &NotFound{id}
	}
	return out, nil
}

func clearDone(ts []Task) []Task {
	return pending(ts)
}

func byPriority(ts []Task) []Task {
	rank := map[Priority]int{High: 0, Med: 1, Low: 2}
	out := pending(ts)
	sort.SliceStable(out, func(i, j int) bool {
		if rank[out[i].pri] != rank[out[j].pri] {
			return rank[out[i].pri] < rank[out[j].pri]
		}
		return out[i].id < out[j].id
	})
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
	return fmt.Sprintf("%s #%d (%s) %s", box(t), t.id, t.pri, t.title)
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
	ts, err := complete(demo(), 1)
	if err != nil {
		fmt.Println(err)
		return
	}
	fmt.Println(renderAll(ts))
}
