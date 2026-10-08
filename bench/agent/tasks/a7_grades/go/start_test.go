package main

import (
	"reflect"
	"testing"
)

func TestCard(t *testing.T) {
	if got := reportCard(Student{"x", []int{50, 60}}); got != "x: 55 (F)" {
		t.Fatalf("got %q", got)
	}
}

func TestRankOrder(t *testing.T) {
	if got := ranking(roster())[:2]; !reflect.DeepEqual(got, []string{"bo", "mia"}) {
		t.Fatalf("got %v", got)
	}
}

func TestLetterB(t *testing.T) {
	if got := letter(85); got != "B" {
		t.Fatalf("got %q", got)
	}
}
