import pytest
from app import *


def test_hidden_sub():
    assert run(Sub(Num(10), Var("x")), {"x": 3}) == "(10 - x) = 7"


def test_hidden_div():
    assert run(Div(Num(7), Num(2)), {}) == "7 / 2 = 3"


def test_hidden_div_zero():
    assert run(Div(Num(1), Sub(Var("x"), Var("x"))), {"x": 4}) == "1 / (x - x) = error: division by zero"


def test_hidden_unbound_still():
    assert run(Sub(Var("w"), Num(1)), {}) == "(w - 1) = error: unbound w"


def test_hidden_eval_neg():
    assert eval_expr(Mul(Sub(Num(2), Num(5)), Num(3)), {}) == -9


def test_hidden_eval_divzero():
    with pytest.raises(DivByZero) as e:
        eval_expr(Div(Num(5), Num(0)), {})
    assert isinstance(e.value, CalcErr)


def test_hidden_simp_sub():
    assert show(simplify(Sub(Var("y"), Num(0)))) == "y"


def test_hidden_simp_div():
    assert show(simplify(Div(Add(Num(0), Var("y")), Num(1)))) == "y"


def test_hidden_simp_inner():
    assert show(simplify(Sub(Mul(Num(1), Var("a")), Var("b")))) == "(a - b)"


def test_hidden_simp_div_inner():
    assert show(simplify(Div(Var("a"), Add(Num(2), Num(0))))) == "a / 2"


def test_hidden_vars():
    assert var_names(Add(Var("x"), Mul(Var("y"), Div(Var("x"), Var("z"))))) == ["x", "y", "z"]


def test_hidden_vars_none():
    assert var_names(Sub(Num(3), Num(1))) == []


def test_hidden_old():
    assert run(Add(Num(2), Mul(Var("x"), Num(4))), {"x": 3}) == "(2 + x * 4) = 14"
