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
        b_val = eval_expr(e.b, env)
        if b_val == 0:
            raise DivByZero()
        return eval_expr(e.a, env) // b_val
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
    seen = set()
    result = []

    def collect(expr: Expr) -> None:
        if isinstance(expr, Num):
            return
        if isinstance(expr, Var):
            if expr.name not in seen:
                seen.add(expr.name)
                result.append(expr.name)
            return
        if isinstance(expr, Add):
            collect(expr.a)
            collect(expr.b)
            return
        if isinstance(expr, Mul):
            collect(expr.a)
            collect(expr.b)
            return
        if isinstance(expr, Sub):
            collect(expr.a)
            collect(expr.b)
            return
        if isinstance(expr, Div):
            collect(expr.a)
            collect(expr.b)
            return

    collect(e)
    return result


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


def test_sub_eval():
    assert eval_expr(Sub(Num(5), Num(3)), {}) == 2


def test_sub_show():
    assert show(Sub(Num(5), Num(3))) == "(5 - 3)"
    assert show(Sub(Var("x"), Num(2))) == "(x - 2)"


def test_sub_simplify():
    assert show(simplify(Sub(Var("x"), Num(0)))) == "x"
    assert show(simplify(Sub(Num(5), Num(0)))) == "5"


def test_div_eval():
    assert eval_expr(Div(Num(10), Num(3)), {}) == 3


def test_div_show():
    assert show(Div(Num(10), Num(3))) == "10 / 3"
    assert show(Div(Var("x"), Num(2))) == "x / 2"


def test_div_simplify():
    assert show(simplify(Div(Var("x"), Num(1)))) == "x"
    assert show(simplify(Div(Num(10), Num(1)))) == "10"


def test_div_by_zero():
    assert run(Div(Num(5), Num(0)), {}) == "5 / 0 = error: division by zero"
    assert run(Div(Num(10), Sub(Num(3), Num(3))), {}) == "10 / (3 - 3) = error: division by zero"


def test_var_names_single():
    assert var_names(Var("x")) == ["x"]


def test_var_names_multiple():
    assert var_names(Add(Var("x"), Var("y"))) == ["x", "y"]
    assert var_names(Add(Var("x"), Var("x"))) == ["x"]


def test_var_names_order():
    assert var_names(Add(Var("a"), Sub(Var("b"), Var("a")))) == ["a", "b"]


def test_var_names_complex():
    expr = Div(Add(Var("x"), Var("y")), Mul(Var("z"), Var("x")))
    assert var_names(expr) == ["x", "y", "z"]


def test_var_names_no_vars():
    assert var_names(Num(42)) == []
    assert var_names(Add(Num(1), Num(2))) == []


def test_run_sub():
    assert run(Sub(Num(10), Num(3)), {}) == "(10 - 3) = 7"
    assert run(Sub(Var("x"), Num(2)), {"x": 5}) == "(x - 2) = 3"


def test_run_div():
    assert run(Div(Num(10), Num(2)), {}) == "10 / 2 = 5"
    assert run(Div(Var("x"), Num(3)), {"x": 9}) == "x / 3 = 3"
