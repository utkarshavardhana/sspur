from dataclasses import dataclass
from typing import Dict, List, Union


@dataclass(frozen=True)
class Num:
    v: int


@dataclass(frozen=True)
class Var:
    name: str


@dataclass(frozen=True)
class Add:
    a: "Expr"
    b: "Expr"


@dataclass(frozen=True)
class Mul:
    a: "Expr"
    b: "Expr"


@dataclass(frozen=True)
class Sub:
    a: "Expr"
    b: "Expr"


@dataclass(frozen=True)
class Div:
    a: "Expr"
    b: "Expr"


Expr = Union[Num, Var, Add, Mul, Sub, Div]


class CalcErr(Exception):
    pass


class Unbound(CalcErr):
    def __init__(self, name: str):
        super().__init__(name)
        self.name = name


class DivByZero(CalcErr):
    def __init__(self):
        super().__init__()


def eval_expr(e: Expr, env: Dict[str, int]) -> int:
    if isinstance(e, Num):
        return e.v
    if isinstance(e, Var):
        if e.name not in env:
            raise Unbound(e.name)
        return env[e.name]
    if isinstance(e, Add):
        return eval_expr(e.a, env) + eval_expr(e.b, env)
    if isinstance(e, Mul):
        return eval_expr(e.a, env) * eval_expr(e.b, env)
    if isinstance(e, Sub):
        return eval_expr(e.a, env) - eval_expr(e.b, env)
    if isinstance(e, Div):
        a = eval_expr(e.a, env)
        b = eval_expr(e.b, env)
        if b == 0:
            raise DivByZero()
        return a // b
    raise TypeError(e)


def show(e: Expr) -> str:
    if isinstance(e, Num):
        return str(e.v)
    if isinstance(e, Var):
        return e.name
    if isinstance(e, Add):
        return f"({show(e.a)} + {show(e.b)})"
    if isinstance(e, Mul):
        return f"{show(e.a)} * {show(e.b)}"
    if isinstance(e, Sub):
        return f"({show(e.a)} - {show(e.b)})"
    if isinstance(e, Div):
        return f"{show(e.a)} / {show(e.b)}"
    raise TypeError(e)


def simplify(e: Expr) -> Expr:
    if isinstance(e, Add):
        if e.a == Num(0):
            return simplify(e.b)
        if e.b == Num(0):
            return simplify(e.a)
        return Add(simplify(e.a), simplify(e.b))
    if isinstance(e, Mul):
        if e.a == Num(1):
            return simplify(e.b)
        if e.b == Num(1):
            return simplify(e.a)
        return Mul(simplify(e.a), simplify(e.b))
    if isinstance(e, Sub):
        if e.b == Num(0):
            return simplify(e.a)
        return Sub(simplify(e.a), simplify(e.b))
    if isinstance(e, Div):
        if e.b == Num(1):
            return simplify(e.a)
        return Div(simplify(e.a), simplify(e.b))
    return e


def var_names(e: Expr) -> List[str]:
    out: List[str] = []

    def go(x: Expr) -> None:
        if isinstance(x, Var):
            if x.name not in out:
                out.append(x.name)
        elif isinstance(x, (Add, Mul, Sub, Div)):
            go(x.a)
            go(x.b)

    go(e)
    return out


def run(e: Expr, env: Dict[str, int]) -> str:
    try:
        return f"{show(e)} = {eval_expr(e, env)}"
    except Unbound as err:
        return f"{show(e)} = error: unbound {err.name}"
    except DivByZero:
        return f"{show(e)} = error: division by zero"


if __name__ == "__main__":
    print(run(Add(Num(2), Mul(Var("x"), Num(4))), {"x": 3}))


def test_run_ok():
    assert run(Add(Num(2), Mul(Var("x"), Num(4))), {"x": 3}) == "(2 + x * 4) = 14"


def test_run_unbound():
    assert run(Var("y"), {}) == "y = error: unbound y"


def test_simplify_ok():
    assert show(simplify(Add(Num(0), Mul(Num(1), Var("z"))))) == "z"


def test_sub_div_eval():
    assert eval_expr(Sub(Num(7), Num(3)), {}) == 4
    assert eval_expr(Div(Num(7), Num(2)), {}) == 3


def test_div_by_zero():
    try:
        eval_expr(Div(Num(1), Num(0)), {})
        assert False
    except DivByZero:
        pass
    assert run(Div(Var("x"), Num(0)), {"x": 5}) == "x / 0 = error: division by zero"


def test_show_sub_div():
    assert show(Sub(Var("a"), Div(Num(6), Var("b")))) == "(a - 6 / b)"
    assert run(Sub(Num(9), Div(Num(6), Num(2))), {}) == "(9 - 6 / 2) = 6"


def test_simplify_sub_div():
    assert simplify(Sub(Add(Num(0), Var("x")), Num(0))) == Var("x")
    assert simplify(Div(Mul(Var("y"), Num(1)), Num(1))) == Var("y")
    assert simplify(Sub(Num(0), Add(Var("x"), Num(0)))) == Sub(Num(0), Var("x"))
    assert simplify(Div(Num(1), Mul(Num(1), Var("x")))) == Div(Num(1), Var("x"))


def test_var_names():
    e = Add(Var("b"), Mul(Sub(Var("a"), Var("b")), Div(Var("c"), Var("a"))))
    assert var_names(e) == ["b", "a", "c"]
    assert var_names(Num(1)) == []
