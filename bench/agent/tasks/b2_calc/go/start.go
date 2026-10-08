package main

import (
	"errors"
	"fmt"
	"strconv"
	"strings"
)

// Tok is a token: Num or Op.
type Tok interface{ tok() }

type Num struct{ v int }

type Op struct{ c string }

func (Num) tok() {}
func (Op) tok()  {}

// CalcErr is implemented by every calculator error.
type CalcErr interface {
	error
	calcErr()
}

type BadChar struct{ ch string }

func (e *BadChar) Error() string { return "bad character " + e.ch }
func (*BadChar) calcErr()        {}

func isDigit(ch rune) bool {
	return strings.ContainsRune("0123456789", ch)
}

func tokenize(s string) ([]Tok, error) {
	toks := []Tok{}
	num := ""
	for _, ch := range s {
		if isDigit(ch) {
			num += string(ch)
			continue
		}
		if num != "" {
			v, _ := strconv.Atoi(num)
			toks = append(toks, Num{v})
			num = ""
		}
		if ch == '+' || ch == '-' {
			toks = append(toks, Op{string(ch)})
		} else if ch != ' ' {
			return nil, &BadChar{string(ch)}
		}
	}
	if num != "" {
		v, _ := strconv.Atoi(num)
		toks = append(toks, Num{v})
	}
	return toks, nil
}

func evalTokens(toks []Tok) int {
	total := 0
	sign := 1
	for _, t := range toks {
		switch t := t.(type) {
		case Num:
			total += sign * t.v
		case Op:
			if t.c == "-" {
				sign = -1
			} else {
				sign = 1
			}
		}
	}
	return total
}

func calc(s string) (int, error) {
	toks, err := tokenize(s)
	if err != nil {
		return 0, err
	}
	return evalTokens(toks), nil
}

func showResult(s string) string {
	v, err := calc(s)
	var bc *BadChar
	switch {
	case err == nil:
		return fmt.Sprintf("%s = %d", s, v)
	case errors.As(err, &bc):
		return fmt.Sprintf("%s = error: bad character %s", s, bc.ch)
	}
	panic(err)
}

func main() {
	fmt.Println(showResult("12 + 30 - 2"))
}
