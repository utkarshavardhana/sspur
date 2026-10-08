package main

import (
	"fmt"
	"reflect"
	"testing"
)

// hShow renders students so that nil and empty score slices compare equal.
func hShow(ss ...Student) string {
	out := ""
	for _, s := range ss {
		out += fmt.Sprintf("%s:%v;", s.name, append([]int{}, s.scores...))
	}
	return out
}

func hScores(t *testing.T, s Student, want ...int) {
	t.Helper()
	if len(s.scores) != len(want) || (len(want) > 0 && !reflect.DeepEqual(s.scores, want)) {
		t.Fatalf("got %v, want %v", s.scores, want)
	}
}

func hStrs(t *testing.T, got []string, want ...string) {
	t.Helper()
	if len(got) != len(want) || (len(want) > 0 && !reflect.DeepEqual(got, want)) {
		t.Fatalf("got %v, want %v", got, want)
	}
}

func TestHidden_empty(t *testing.T) {
	if got := average(Student{"e", []int{}}); got != 0 {
		t.Fatalf("got %d", got)
	}
	if got := reportCard(Student{"e", []int{}}); got != "e: 0 (F)" {
		t.Fatalf("got %q", got)
	}
}

func TestHidden_bounds(t *testing.T) {
	got := []string{}
	for _, x := range []int{90, 89, 80, 70, 60, 59, 100} {
		got = append(got, letter(x))
	}
	hStrs(t, got, "A", "B", "B", "C", "D", "F", "A")
}

func TestHidden_rank(t *testing.T) {
	hStrs(t, ranking(roster()), "bo", "mia", "zed", "ali")
}

func TestHidden_rank_ties(t *testing.T) {
	hStrs(t, ranking([]Student{{"cy", []int{80}}, {"al", []int{80}}, {"bo", []int{90}}}), "bo", "al", "cy")
}

func TestHidden_drop(t *testing.T) {
	hScores(t, dropLowest(Student{"z", []int{95, 50, 90, 50}}), 95, 90, 50)
}

func TestHidden_drop_one(t *testing.T) {
	hScores(t, dropLowest(Student{"z", []int{70}}), 70)
	hScores(t, dropLowest(Student{"z", []int{}}))
}

func TestHidden_drop_name(t *testing.T) {
	if got, want := hShow(dropLowest(Student{"q", []int{1, 2}})), hShow(Student{"q", []int{2}}); got != want {
		t.Fatalf("got %s, want %s", got, want)
	}
}

func TestHidden_curve(t *testing.T) {
	got := hShow(curve([]Student{{"a", []int{95, 80}}, {"b", []int{}}}, 10)...)
	if want := hShow(Student{"a", []int{100, 90}}, Student{"b", []int{}}); got != want {
		t.Fatalf("got %s, want %s", got, want)
	}
}

func TestHidden_honor(t *testing.T) {
	hStrs(t, honorRoll(roster()), "bo", "mia", "zed")
}

func TestHidden_honor_empty(t *testing.T) {
	hStrs(t, honorRoll([]Student{}))
}

func TestHidden_grade(t *testing.T) {
	if got := grade(Student{"m", []int{90, 90}}); got != "A" {
		t.Fatalf("got %q", got)
	}
}
