from dataclasses import dataclass
from typing import Dict, Union


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


Expr = Union[Num, Var, Add, Mul]


class CalcErr(Exception):
    pass


class Unbound(CalcErr):
    def __init__(self, name: str):
        super().__init__(name)
        self.name = name


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
    return e


def run(e: Expr, env: Dict[str, int]) -> str:
    try:
        return f"{show(e)} = {eval_expr(e, env)}"
    except Unbound as err:
        return f"{show(e)} = error: unbound {err.name}"


if __name__ == "__main__":
    print(run(Add(Num(2), Mul(Var("x"), Num(4))), {"x": 3}))


def test_run_ok():
    assert run(Add(Num(2), Mul(Var("x"), Num(4))), {"x": 3}) == "(2 + x * 4) = 14"


def test_run_unbound():
    assert run(Var("y"), {}) == "y = error: unbound y"


def test_simplify_ok():
    assert show(simplify(Add(Num(0), Mul(Num(1), Var("z"))))) == "z"
