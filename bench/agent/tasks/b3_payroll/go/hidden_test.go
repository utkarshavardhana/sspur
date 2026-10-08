package main

import (
	"errors"
	"fmt"
	"reflect"
	"testing"
)

func hErr(text string) string {
	rows, err := parse(text)
	if err == nil {
		return fmt.Sprintf("rows %d", len(rows))
	}
	var br *BadRow
	var bn *BadNumber
	var ce CsvErr
	switch {
	case errors.As(err, &br):
		return fmt.Sprintf("row %d", br.line)
	case errors.As(err, &bn):
		return fmt.Sprintf("number %d %s", bn.line, bn.field)
	case errors.As(err, &ce):
		return "other"
	}
	return "unexpected " + err.Error()
}

func hParse(t *testing.T, text string) []Row {
	t.Helper()
	rows, err := parse(text)
	if err != nil {
		t.Fatalf("unexpected error %v", err)
	}
	return rows
}

func hExpect(t *testing.T, got, want string) {
	t.Helper()
	if got != want {
		t.Fatalf("got %q, want %q", got, want)
	}
}

func hPay(t *testing.T, r Row, want int) {
	t.Helper()
	if got := pay(r); got != want {
		t.Fatalf("pay(%v) = %d, want %d", r, got, want)
	}
}

func TestHidden_sample(t *testing.T) {
	rows := hParse(t, sample())
	if len(rows) != 6 {
		t.Fatalf("len = %d", len(rows))
	}
	hPay(t, rows[2], 4000)
}

func TestHidden_header(t *testing.T) {
	hExpect(t, hErr("name,rate,hours\nBeth,400,10"), "rows 1")
}

func TestHidden_header_trimmed(t *testing.T) {
	hExpect(t, hErr("  name,rate,hours  \nBeth,400,10"), "rows 1")
}

func TestHidden_header_not_first(t *testing.T) {
	hExpect(t, hErr("Beth,400,10\nname,rate,hours"), "number 2 rate")
}

func TestHidden_blank_lines(t *testing.T) {
	hExpect(t, hErr("\nBeth,400,10\n\n   \nDan,375,2\n"), "rows 2")
	hExpect(t, hErr(""), "rows 0")
}

func TestHidden_trims(t *testing.T) {
	if got := hParse(t, " Beth , 400 , 10 "); !reflect.DeepEqual(got, []Row{{"Beth", 400, 10}}) {
		t.Fatalf("got %v", got)
	}
}

func TestHidden_bad_row(t *testing.T) {
	hExpect(t, hErr("Beth,400\nDan,375,2"), "row 1")
	hExpect(t, hErr("a,1,2\nb,1,2,3"), "row 2")
}

func TestHidden_line_numbers(t *testing.T) {
	hExpect(t, hErr("name,rate,hours\n\nBeth,4x,1"), "number 3 rate")
}

func TestHidden_bad_hours(t *testing.T) {
	hExpect(t, hErr("Beth,400,ten"), "number 1 hours")
	hExpect(t, hErr("Beth,400,-5"), "number 1 hours")
}

func TestHidden_rate_first(t *testing.T) {
	hExpect(t, hErr("Beth,x,y"), "number 1 rate")
	hExpect(t, hErr("Beth,-1,3"), "number 1 rate")
}

func TestHidden_overtime(t *testing.T) {
	hPay(t, Row{"x", 400, 45}, 19000)
	hPay(t, Row{"y", 5, 41}, 207)
}

func TestHidden_no_overtime(t *testing.T) {
	hPay(t, Row{"x", 400, 40}, 16000)
	hPay(t, Row{"z", 375, 0}, 0)
}

func TestHidden_report_sorted(t *testing.T) {
	hExpect(t, report(hParse(t, "Beth,400,0\nDan,375,0\nKathy,400,10")), "Kathy: 4000\nBeth: 0\nDan: 0\ntotal: 4000")
}

func TestHidden_report_overtime(t *testing.T) {
	hExpect(t, report([]Row{{"a", 10, 50}, {"b", 20, 30}}), "b: 600\na: 550\ntotal: 1150")
}

func TestHidden_report_empty(t *testing.T) {
	hExpect(t, report([]Row{}), "total: 0")
}

func TestHidden_validate(t *testing.T) {
	got := validate("name,rate,hours\nBeth,400\nDan,x,1\nKathy,400,10\nMark,500,z")
	want := []string{"line 2: expected 3 fields", "line 3: bad rate", "line 5: bad hours"}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("got %q", got)
	}
}

func TestHidden_validate_ok(t *testing.T) {
	if got := validate(sample()); len(got) != 0 {
		t.Fatalf("got %q", got)
	}
	if got := validate(""); len(got) != 0 {
		t.Fatalf("got %q", got)
	}
}
