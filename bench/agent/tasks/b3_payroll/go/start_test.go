package main

import "testing"

func TestParsed(t *testing.T) {
	rows := parse(sample())
	if len(rows) != 6 || rows[2] != (Row{"Kathy", 400, 10}) {
		t.Fatalf("got %v", rows)
	}
}

func TestPaid(t *testing.T) {
	if got := pay(Row{"Mark", 500, 20}); got != 10000 {
		t.Fatalf("got %d", got)
	}
}

func TestReported(t *testing.T) {
	if got := report(parse("Beth,400,0\nKathy,400,10")); got != "Beth: 0\nKathy: 4000" {
		t.Fatalf("got %q", got)
	}
}
