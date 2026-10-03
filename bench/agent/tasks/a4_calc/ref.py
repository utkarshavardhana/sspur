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
    pass


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
        a, b = eval_expr(e.a, env), eval_expr(e.b, env)
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


def run(e: Expr, env: Dict[str, int]) -> str:
    try:
        return f"{show(e)} = {eval_expr(e, env)}"
    except Unbound as err:
        return f"{show(e)} = error: unbound {err.name}"
    except DivByZero:
        return f"{show(e)} = error: division by zero"


def var_names(e: Expr) -> List[str]:
    out: List[str] = []

    def walk(x: Expr) -> None:
        if isinstance(x, Var):
            if x.name not in out:
                out.append(x.name)
        elif not isinstance(x, Num):
            walk(x.a)
            walk(x.b)

    walk(e)
    return out


if __name__ == "__main__":
    print(run(Add(Num(2), Mul(Var("x"), Num(4))), {"x": 3}))


def test_run_ok():
    assert run(Add(Num(2), Mul(Var("x"), Num(4))), {"x": 3}) == "(2 + x * 4) = 14"


def test_run_unbound():
    assert run(Var("y"), {}) == "y = error: unbound y"


def test_simplify_ok():
    assert show(simplify(Add(Num(0), Mul(Num(1), Var("z"))))) == "z"
