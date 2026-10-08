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

type DivByZero struct{}

func (*DivByZero) Error() string { return "division by zero" }
func (*DivByZero) calcErr()      {}

type BadSyntax struct{}

func (*BadSyntax) Error() string { return "syntax" }
func (*BadSyntax) calcErr()      {}

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
		if strings.ContainsRune("+-*/()", ch) {
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

func isOp(toks []Tok, i int, c string) bool {
	if i >= len(toks) {
		return false
	}
	o, ok := toks[i].(Op)
	return ok && o.c == c
}

// apply truncates division toward zero, which is Go's native integer division.
func apply(c string, a, b int) (int, error) {
	switch c {
	case "+":
		return a + b, nil
	case "-":
		return a - b, nil
	case "*":
		return a * b, nil
	}
	if b == 0 {
		return 0, &DivByZero{}
	}
	return a / b, nil
}

func parseExpr(toks []Tok, i int) (int, int, error) {
	acc, i, err := parseTerm(toks, i)
	for err == nil && (isOp(toks, i, "+") || isOp(toks, i, "-")) {
		c := toks[i].(Op).c
		var b int
		if b, i, err = parseTerm(toks, i+1); err == nil {
			acc, err = apply(c, acc, b)
		}
	}
	return acc, i, err
}

func parseTerm(toks []Tok, i int) (int, int, error) {
	acc, i, err := parseUnary(toks, i)
	for err == nil && (isOp(toks, i, "*") || isOp(toks, i, "/")) {
		c := toks[i].(Op).c
		var b int
		if b, i, err = parseUnary(toks, i+1); err == nil {
			acc, err = apply(c, acc, b)
		}
	}
	return acc, i, err
}

func parseUnary(toks []Tok, i int) (int, int, error) {
	if isOp(toks, i, "-") {
		v, j, err := parseUnary(toks, i+1)
		return -v, j, err
	}
	return parseAtom(toks, i)
}

func parseAtom(toks []Tok, i int) (int, int, error) {
	if i < len(toks) {
		if n, ok := toks[i].(Num); ok {
			return n.v, i + 1, nil
		}
	}
	if isOp(toks, i, "(") {
		v, j, err := parseExpr(toks, i+1)
		if err != nil {
			return 0, 0, err
		}
		if !isOp(toks, j, ")") {
			return 0, 0, &BadSyntax{}
		}
		return v, j + 1, nil
	}
	return 0, 0, &BadSyntax{}
}

func calc(s string) (int, error) {
	toks, err := tokenize(s)
	if err != nil {
		return 0, err
	}
	v, i, err := parseExpr(toks, 0)
	if err != nil {
		return 0, err
	}
	if i != len(toks) {
		return 0, &BadSyntax{}
	}
	return v, nil
}

func showResult(s string) string {
	v, err := calc(s)
	var bc *BadChar
	var dz *DivByZero
	var bs *BadSyntax
	switch {
	case err == nil:
		return fmt.Sprintf("%s = %d", s, v)
	case errors.As(err, &bc):
		return fmt.Sprintf("%s = error: bad character %s", s, bc.ch)
	case errors.As(err, &dz):
		return s + " = error: division by zero"
	case errors.As(err, &bs):
		return s + " = error: syntax"
	}
	panic(err)
}

func main() {
	fmt.Println(showResult("12 + 30 - 2"))
}
