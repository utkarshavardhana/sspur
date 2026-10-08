package main

import (
	"fmt"
	"sort"
	"strings"
)

type Student struct {
	name   string
	scores []int
}

func average(s Student) int {
	sum := 0
	for _, x := range s.scores {
		sum += x
	}
	return sum / len(s.scores)
}

func letter(avg int) string {
	switch {
	case avg > 90:
		return "A"
	case avg > 80:
		return "B"
	case avg > 70:
		return "C"
	case avg > 60:
		return "D"
	}
	return "F"
}

func grade(s Student) string {
	return letter(average(s))
}

func ranking(ss []Student) []string {
	sorted := append([]Student{}, ss...)
	sort.SliceStable(sorted, func(i, j int) bool {
		return average(sorted[i]) > average(sorted[j])
	})
	names := make([]string, len(sorted))
	for k, s := range sorted {
		names[k] = s.name
	}
	return names
}

func reportCard(s Student) string {
	return fmt.Sprintf("%s: %d (%s)", s.name, average(s), grade(s))
}

func roster() []Student {
	return []Student{
		{"mia", []int{88, 92, 95}},
		{"ali", []int{70, 65, 80}},
		{"zed", []int{95, 90, 50}},
		{"bo", []int{100, 92, 89}},
	}
}

func main() {
	cards := []string{}
	for _, s := range roster() {
		cards = append(cards, reportCard(s))
	}
	fmt.Println(strings.Join(cards, "\n"))
}
