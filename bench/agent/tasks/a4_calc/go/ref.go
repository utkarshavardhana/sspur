package main

import (
	"errors"
	"fmt"
	"slices"
	"strconv"
)

// Expr is implemented by every expression variant.
type Expr interface{ isExpr() }

type Num struct{ v int }
type Var struct{ name string }
type Add struct{ a, b Expr }
type Mul struct{ a, b Expr }
type Sub struct{ a, b Expr }
type Div struct{ a, b Expr }

func (Num) isExpr() {}
func (Var) isExpr() {}
func (Add) isExpr() {}
func (Mul) isExpr() {}
func (Sub) isExpr() {}
func (Div) isExpr() {}

// CalcErr is implemented by every calculator error.
type CalcErr interface {
	error
	calcErr()
}

type Unbound struct{ name string }

func (e *Unbound) Error() string { return "unbound " + e.name }
func (*Unbound) calcErr()        {}

type DivByZero struct{}

func (*DivByZero) Error() string { return "division by zero" }
func (*DivByZero) calcErr()      {}

func eval2(x, y Expr, env map[string]int) (int, int, error) {
	a, err := evalExpr(x, env)
	if err != nil {
		return 0, 0, err
	}
	b, err := evalExpr(y, env)
	if err != nil {
		return 0, 0, err
	}
	return a, b, nil
}

func evalExpr(e Expr, env map[string]int) (int, error) {
	switch e := e.(type) {
	case Num:
		return e.v, nil
	case Var:
		v, ok := env[e.name]
		if !ok {
			return 0, &Unbound{e.name}
		}
		return v, nil
	case Add:
		a, b, err := eval2(e.a, e.b, env)
		return a + b, err
	case Mul:
		a, b, err := eval2(e.a, e.b, env)
		return a * b, err
	case Sub:
		a, b, err := eval2(e.a, e.b, env)
		return a - b, err
	case Div:
		a, b, err := eval2(e.a, e.b, env)
		if err != nil {
			return 0, err
		}
		if b == 0 {
			return 0, &DivByZero{}
		}
		q := a / b
		if a%b != 0 && (a < 0) != (b < 0) {
			q-- // floor division, as in Python
		}
		return q, nil
	}
	panic(fmt.Sprintf("unknown expr %#v", e))
}

func show(e Expr) string {
	switch e := e.(type) {
	case Num:
		return strconv.Itoa(e.v)
	case Var:
		return e.name
	case Add:
		return "(" + show(e.a) + " + " + show(e.b) + ")"
	case Mul:
		return show(e.a) + " * " + show(e.b)
	case Sub:
		return "(" + show(e.a) + " - " + show(e.b) + ")"
	case Div:
		return show(e.a) + " / " + show(e.b)
	}
	panic(fmt.Sprintf("unknown expr %#v", e))
}

func simplify(e Expr) Expr {
	switch e := e.(type) {
	case Add:
		if e.a == (Num{0}) {
			return simplify(e.b)
		}
		if e.b == (Num{0}) {
			return simplify(e.a)
		}
		return Add{simplify(e.a), simplify(e.b)}
	case Mul:
		if e.a == (Num{1}) {
			return simplify(e.b)
		}
		if e.b == (Num{1}) {
			return simplify(e.a)
		}
		return Mul{simplify(e.a), simplify(e.b)}
	case Sub:
		if e.b == (Num{0}) {
			return simplify(e.a)
		}
		return Sub{simplify(e.a), simplify(e.b)}
	case Div:
		if e.b == (Num{1}) {
			return simplify(e.a)
		}
		return Div{simplify(e.a), simplify(e.b)}
	}
	return e
}

func run(e Expr, env map[string]int) string {
	v, err := evalExpr(e, env)
	var ub *Unbound
	var dz *DivByZero
	switch {
	case err == nil:
		return fmt.Sprintf("%s = %d", show(e), v)
	case errors.As(err, &ub):
		return fmt.Sprintf("%s = error: unbound %s", show(e), ub.name)
	case errors.As(err, &dz):
		return fmt.Sprintf("%s = error: division by zero", show(e))
	}
	panic(err)
}

func varNames(e Expr) []string {
	out := []string{}
	var walk func(x Expr)
	walk = func(x Expr) {
		switch x := x.(type) {
		case Var:
			if !slices.Contains(out, x.name) {
				out = append(out, x.name)
			}
		case Add:
			walk(x.a)
			walk(x.b)
		case Mul:
			walk(x.a)
			walk(x.b)
		case Sub:
			walk(x.a)
			walk(x.b)
		case Div:
			walk(x.a)
			walk(x.b)
		}
	}
	walk(e)
	return out
}

func main() {
	fmt.Println(run(Add{Num{2}, Mul{Var{"x"}, Num{4}}}, map[string]int{"x": 3}))
}
