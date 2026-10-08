package main

import (
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

func sample() string {
	return "Beth,400,0\nDan,375,0\nKathy,400,10\nMark,500,20\nMary,550,22\nSusie,425,18"
}

type CsvErr interface {
	error
	csvErr()
}

type BadRow struct{ line int }

func (e *BadRow) Error() string { return fmt.Sprintf("line %d: expected 3 fields", e.line) }
func (e *BadRow) csvErr()       {}

type BadNumber struct {
	line  int
	field string
}

func (e *BadNumber) Error() string { return fmt.Sprintf("line %d: bad %s", e.line, e.field) }
func (e *BadNumber) csvErr()       {}

var intPat = regexp.MustCompile(`^[0-9]+$`)

func toNum(s string) (int, bool) {
	if !intPat.MatchString(s) {
		return 0, false
	}
	n, err := strconv.Atoi(s)
	if err != nil {
		return 0, false
	}
	return n, true
}

func parseLine(n int, line string) (Row, CsvErr) {
	f := strings.Split(line, ",")
	if len(f) != 3 {
		return Row{}, &BadRow{n}
	}
	for i := range f {
		f[i] = strings.TrimSpace(f[i])
	}
	rate, ok := toNum(f[1])
	if !ok {
		return Row{}, &BadNumber{n, "rate"}
	}
	hours, ok := toNum(f[2])
	if !ok {
		return Row{}, &BadNumber{n, "hours"}
	}
	return Row{f[0], rate, hours}, nil
}

func scan(text string, each func(n int, line string)) {
	for i, line := range strings.Split(text, "\n") {
		t := strings.TrimSpace(line)
		if t == "" || (i == 0 && t == "name,rate,hours") {
			continue
		}
		each(i+1, t)
	}
}

func parse(text string) ([]Row, error) {
	rows := []Row{}
	var err error
	scan(text, func(n int, line string) {
		if err != nil {
			return
		}
		r, e := parseLine(n, line)
		if e != nil {
			err = e
			return
		}
		rows = append(rows, r)
	})
	if err != nil {
		return nil, err
	}
	return rows, nil
}

func validate(text string) []string {
	msgs := []string{}
	scan(text, func(n int, line string) {
		if _, e := parseLine(n, line); e != nil {
			msgs = append(msgs, e.Error())
		}
	})
	return msgs
}

func pay(r Row) int {
	p := r.rate * r.hours
	if r.hours > 40 {
		p = r.rate*40 + r.rate*3*(r.hours-40)/2
	}
	return p
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

func main() {
	rows, _ := parse(sample())
	fmt.Println(report(rows))
}
