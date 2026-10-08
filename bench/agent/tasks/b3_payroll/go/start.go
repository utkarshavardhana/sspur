package main

import (
	"fmt"
	"regexp"
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

var intPat = regexp.MustCompile(`^[+-]?[0-9]+(_[0-9]+)*$`)

// toInt parses an integer the way Python's int() does (surrounding whitespace, an
// optional sign, digits) and gives 0 for anything else.
func toInt(s string) int {
	t := strings.TrimSpace(s)
	if !intPat.MatchString(t) {
		return 0
	}
	n, err := strconv.Atoi(strings.ReplaceAll(t, "_", ""))
	if err != nil {
		return 0
	}
	return n
}

func parseRow(line string) Row {
	f := strings.Split(line, ",")
	return Row{f[0], toInt(f[1]), toInt(f[2])}
}

func parse(text string) []Row {
	rows := []Row{}
	for _, line := range strings.Split(text, "\n") {
		rows = append(rows, parseRow(line))
	}
	return rows
}

func pay(r Row) int {
	return r.rate * r.hours
}

func report(rows []Row) string {
	lines := []string{}
	for _, r := range rows {
		lines = append(lines, fmt.Sprintf("%s: %d", r.name, pay(r)))
	}
	return strings.Join(lines, "\n")
}

func main() {
	fmt.Println(report(parse(sample())))
}
