package main

import (
	"errors"
	"fmt"
	"regexp"
	"sort"
	"strconv"
	"strings"
)

type Row struct {
	name  string
	rate  int
	hours int
}

// CsvErr is implemented by every CSV error.
type CsvErr interface {
	error
	csvErr()
}

type BadRow struct{ line int }

func (e *BadRow) Error() string { return fmt.Sprintf("line %d: expected 3 fields", e.line) }
func (*BadRow) csvErr()         {}

type BadNumber struct {
	line  int
	field string
}

func (e *BadNumber) Error() string { return fmt.Sprintf("line %d: bad %s", e.line, e.field) }
func (*BadNumber) csvErr()         {}

func sample() string {
	return "Beth,400,0\nDan,375,0\nKathy,400,10\nMark,500,20\nMary,550,22\nSusie,425,18"
}

var intPat = regexp.MustCompile(`^[+-]?[0-9]+(_[0-9]+)*$`)

// toInt parses an integer the way Python's int() does (surrounding whitespace, an
// optional sign, digits).
func toInt(s string) (int, bool) {
	t := strings.TrimSpace(s)
	if !intPat.MatchString(t) {
		return 0, false
	}
	n, err := strconv.Atoi(strings.ReplaceAll(t, "_", ""))
	return n, err == nil
}

func num(s string, line int, field string) (int, error) {
	n, ok := toInt(s)
	if !ok || n < 0 {
		return 0, &BadNumber{line, field}
	}
	return n, nil
}

func parseRow(line string, n int) (Row, error) {
	f := strings.Split(line, ",")
	for k := range f {
		f[k] = strings.TrimSpace(f[k])
	}
	if len(f) != 3 {
		return Row{}, &BadRow{n}
	}
	rate, err := num(f[1], n, "rate")
	if err != nil {
		return Row{}, err
	}
	hours, err := num(f[2], n, "hours")
	if err != nil {
		return Row{}, err
	}
	return Row{f[0], rate, hours}, nil
}

type dataLine struct {
	n    int
	line string
}

func dataLines(text string) []dataLine {
	out := []dataLine{}
	for k, line := range strings.Split(text, "\n") {
		i := k + 1
		t := strings.TrimSpace(line)
		if t == "" || (i == 1 && t == "name,rate,hours") {
			continue
		}
		out = append(out, dataLine{i, line})
	}
	return out
}

func parse(text string) ([]Row, error) {
	rows := []Row{}
	for _, d := range dataLines(text) {
		r, err := parseRow(d.line, d.n)
		if err != nil {
			return nil, err
		}
		rows = append(rows, r)
	}
	return rows, nil
}

func pay(r Row) int {
	if r.hours > 40 {
		return r.rate*40 + r.rate*3*(r.hours-40)/2
	}
	return r.rate * r.hours
}

func report(rows []Row) string {
	sorted := append([]Row{}, rows...)
	sort.SliceStable(sorted, func(i, j int) bool {
		pi, pj := pay(sorted[i]), pay(sorted[j])
		if pi != pj {
			return pi > pj
		}
		return sorted[i].name < sorted[j].name
	})
	lines := []string{}
	total := 0
	for _, r := range sorted {
		lines = append(lines, fmt.Sprintf("%s: %d", r.name, pay(r)))
		total += pay(r)
	}
	lines = append(lines, fmt.Sprintf("total: %d", total))
	return strings.Join(lines, "\n")
}

func validate(text string) []string {
	out := []string{}
	for _, d := range dataLines(text) {
		_, err := parseRow(d.line, d.n)
		var br *BadRow
		var bn *BadNumber
		switch {
		case errors.As(err, &br):
			out = append(out, fmt.Sprintf("line %d: expected 3 fields", br.line))
		case errors.As(err, &bn):
			out = append(out, fmt.Sprintf("line %d: bad %s", bn.line, bn.field))
		}
	}
	return out
}

func main() {
	rows, err := parse(sample())
	if err != nil {
		panic(err)
	}
	fmt.Println(report(rows))
}
