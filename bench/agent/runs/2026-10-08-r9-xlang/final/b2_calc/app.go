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

type DivByZero struct{}

func (*DivByZero) Error() string { return "division by zero" }
func (*DivByZero) calcErr()      {}

type BadSyntax struct{}

func (*BadSyntax) Error() string { return "syntax" }
func (*BadSyntax) calcErr()      {}

type node func() (int, error)

type parser struct {
	toks []Tok
	pos  int
}

func (p *parser) peekOp() string {
	if p.pos < len(p.toks) {
		if o, ok := p.toks[p.pos].(Op); ok {
			return o.c
		}
	}
	return ""
}

func (p *parser) expr() (node, error) {
	left, err := p.term()
	if err != nil {
		return nil, err
	}
	for {
		op := p.peekOp()
		if op != "+" && op != "-" {
			return left, nil
		}
		p.pos++
		right, err := p.term()
		if err != nil {
			return nil, err
		}
		l, r := left, right
		left = func() (int, error) {
			a, err := l()
			if err != nil {
				return 0, err
			}
			b, err := r()
			if err != nil {
				return 0, err
			}
			if op == "+" {
				return a + b, nil
			}
			return a - b, nil
		}
	}
}

func (p *parser) term() (node, error) {
	left, err := p.unary()
	if err != nil {
		return nil, err
	}
	for {
		op := p.peekOp()
		if op != "*" && op != "/" {
			return left, nil
		}
		p.pos++
		right, err := p.unary()
		if err != nil {
			return nil, err
		}
		l, r := left, right
		left = func() (int, error) {
			a, err := l()
			if err != nil {
				return 0, err
			}
			b, err := r()
			if err != nil {
				return 0, err
			}
			if op == "*" {
				return a * b, nil
			}
			if b == 0 {
				return 0, &DivByZero{}
			}
			return a / b, nil
		}
	}
}

func (p *parser) unary() (node, error) {
	if p.peekOp() == "-" {
		p.pos++
		x, err := p.unary()
		if err != nil {
			return nil, err
		}
		return func() (int, error) {
			v, err := x()
			return -v, err
		}, nil
	}
	return p.atom()
}

func (p *parser) atom() (node, error) {
	if p.pos >= len(p.toks) {
		return nil, &BadSyntax{}
	}
	t := p.toks[p.pos]
	p.pos++
	switch t := t.(type) {
	case Num:
		v := t.v
		return func() (int, error) { return v, nil }, nil
	case Op:
		if t.c == "(" {
			n, err := p.expr()
			if err != nil {
				return nil, err
			}
			if p.peekOp() != ")" {
				return nil, &BadSyntax{}
			}
			p.pos++
			return n, nil
		}
	}
	return nil, &BadSyntax{}
}

func evalTokens(toks []Tok) (int, error) {
	p := &parser{toks: toks}
	n, err := p.expr()
	if err != nil {
		return 0, err
	}
	if p.pos != len(toks) {
		return 0, &BadSyntax{}
	}
	return n()
}

func calc(s string) (int, error) {
	toks, err := tokenize(s)
	if err != nil {
		return 0, err
	}
	return evalTokens(toks)
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
		return fmt.Sprintf("%s = error: division by zero", s)
	case errors.As(err, &bs):
		return fmt.Sprintf("%s = error: syntax", s)
	}
	panic(err)
}

func main() {
	fmt.Println(showResult("12 + 30 - 2"))
}
