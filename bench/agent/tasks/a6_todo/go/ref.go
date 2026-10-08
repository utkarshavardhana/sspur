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

// TaskErr is implemented by every task error.
type TaskErr interface {
	error
	taskErr()
}

type NotFound struct{ id int }

func (e *NotFound) Error() string { return fmt.Sprintf("no task %d", e.id) }
func (*NotFound) taskErr()        {}

type AlreadyDone struct{ id int }

func (e *AlreadyDone) Error() string { return fmt.Sprintf("task %d already done", e.id) }
func (*AlreadyDone) taskErr()        {}

func findTask(ts []Task, id int) (Task, error) {
	for _, t := range ts {
		if t.id == id {
			return t, nil
		}
	}
	return Task{}, &NotFound{id}
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

func complete(ts []Task, id int) ([]Task, error) {
	f, err := findTask(ts, id)
	if err != nil {
		return nil, err
	}
	if f.done {
		return nil, &AlreadyDone{id}
	}
	out := make([]Task, len(ts))
	for k, t := range ts {
		if t.id == id {
			t.done = true
		}
		out[k] = t
	}
	return out, nil
}

func retitle(ts []Task, id int, title string) ([]Task, error) {
	if _, err := findTask(ts, id); err != nil {
		return nil, err
	}
	out := make([]Task, len(ts))
	for k, t := range ts {
		if t.id == id {
			t.title = title
		}
		out[k] = t
	}
	return out, nil
}

func clearDone(ts []Task) []Task {
	return pending(ts)
}

var rank = map[Priority]int{High: 0, Med: 1, Low: 2}

func byPriority(ts []Task) []Task {
	out := pending(ts)
	sort.SliceStable(out, func(i, j int) bool {
		a, b := out[i], out[j]
		if rank[a.pri] != rank[b.pri] {
			return rank[a.pri] < rank[b.pri]
		}
		return a.id < b.id
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
