package main

import (
	"errors"
	"reflect"
	"testing"
)

func hIds(ts []Task) []int {
	out := []int{}
	for _, t := range ts {
		out = append(out, t.id)
	}
	return out
}

func hMust(t *testing.T, ts []Task, err error) []Task {
	t.Helper()
	if err != nil {
		t.Fatalf("unexpected error %v", err)
	}
	return ts
}

func hComplete(t *testing.T, ts []Task, id int) []Task {
	t.Helper()
	out, err := complete(ts, id)
	return hMust(t, out, err)
}

func hExpectIds(t *testing.T, ts []Task, want ...int) {
	t.Helper()
	if got := hIds(ts); !reflect.DeepEqual(got, want) {
		t.Fatalf("got %v, want %v", got, want)
	}
}

func TestHidden_complete_ok(t *testing.T) {
	hExpectIds(t, pending(hComplete(t, demo(), 2)), 1, 3, 4)
}

func TestHidden_complete_missing(t *testing.T) {
	_, err := complete(demo(), 9)
	var e *NotFound
	if !errors.As(err, &e) || e.id != 9 {
		t.Fatalf("err = %v", err)
	}
	var te TaskErr
	if !errors.As(err, &te) {
		t.Fatalf("not a TaskErr: %v", err)
	}
}

func TestHidden_complete_twice(t *testing.T) {
	_, err := complete(hComplete(t, demo(), 2), 2)
	var e *AlreadyDone
	if !errors.As(err, &e) || e.id != 2 {
		t.Fatalf("err = %v", err)
	}
	var te TaskErr
	if !errors.As(err, &te) {
		t.Fatalf("not a TaskErr: %v", err)
	}
}

func TestHidden_render_high(t *testing.T) {
	if got := render(Task{2, "write report", false, High}); got != "[ ] #2 (high) write report" {
		t.Fatalf("got %q", got)
	}
}

func TestHidden_render_low(t *testing.T) {
	if got := render(Task{5, "nap", true, Low}); got != "[x] #5 (low) nap" {
		t.Fatalf("got %q", got)
	}
}

func TestHidden_render_all(t *testing.T) {
	want := "[ ] #1 (low) buy milk\n[ ] #2 (high) write report\n[ ] #3 (med) call bob\n[ ] #4 (high) fix sink"
	if got := renderAll(demo()); got != want {
		t.Fatalf("got %q", got)
	}
}

func TestHidden_by_pri(t *testing.T) {
	hExpectIds(t, byPriority(demo()), 2, 4, 3, 1)
}

func TestHidden_by_pri_pending(t *testing.T) {
	hExpectIds(t, byPriority(hComplete(t, demo(), 4)), 2, 3, 1)
}

func TestHidden_clear_done(t *testing.T) {
	hExpectIds(t, clearDone(hComplete(t, hComplete(t, demo(), 1), 3)), 2, 4)
}

func TestHidden_retitle(t *testing.T) {
	ts, err := retitle(demo(), 3, "call alice")
	got := []string{}
	for _, x := range hMust(t, ts, err) {
		got = append(got, x.title)
	}
	if !reflect.DeepEqual(got, []string{"buy milk", "write report", "call alice", "fix sink"}) {
		t.Fatalf("got %v", got)
	}
}

func TestHidden_retitle_missing(t *testing.T) {
	_, err := retitle(demo(), 7, "x")
	var e *NotFound
	if !errors.As(err, &e) || e.id != 7 {
		t.Fatalf("err = %v", err)
	}
}
