from dataclasses import dataclass
from typing import List, Tuple, Union


@dataclass(frozen=True)
class Num:
    v: int


@dataclass(frozen=True)
class Op:
    c: str


Tok = Union[Num, Op]


class CalcErr(Exception):
    pass


class BadChar(CalcErr):
    def __init__(self, ch: str):
        super().__init__(ch)
        self.ch = ch


class DivByZero(CalcErr):
    pass


class BadSyntax(CalcErr):
    pass


def is_digit(ch: str) -> bool:
    return ch in "0123456789"


def tokenize(s: str) -> List[Tok]:
    toks: List[Tok] = []
    num = ""
    for ch in s:
        if is_digit(ch):
            num += ch
        else:
            if num:
                toks.append(Num(int(num)))
                num = ""
            if ch in "+-*/()":
                toks.append(Op(ch))
            elif ch != " ":
                raise BadChar(ch)
    if num:
        toks.append(Num(int(num)))
    return toks


def _is(toks: List[Tok], i: int, c: str) -> bool:
    return i < len(toks) and toks[i] == Op(c)


def _apply(c: str, a: int, b: int) -> int:
    if c == "+":
        return a + b
    if c == "-":
        return a - b
    if c == "*":
        return a * b
    if b == 0:
        raise DivByZero()
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b >= 0) else -q


def _expr(toks: List[Tok], i: int) -> Tuple[int, int]:
    acc, i = _term(toks, i)
    while _is(toks, i, "+") or _is(toks, i, "-"):
        c = toks[i].c
        b, i = _term(toks, i + 1)
        acc = _apply(c, acc, b)
    return acc, i


def _term(toks: List[Tok], i: int) -> Tuple[int, int]:
    acc, i = _unary(toks, i)
    while _is(toks, i, "*") or _is(toks, i, "/"):
        c = toks[i].c
        b, i = _unary(toks, i + 1)
        acc = _apply(c, acc, b)
    return acc, i


def _unary(toks: List[Tok], i: int) -> Tuple[int, int]:
    if _is(toks, i, "-"):
        v, i = _unary(toks, i + 1)
        return -v, i
    return _atom(toks, i)


def _atom(toks: List[Tok], i: int) -> Tuple[int, int]:
    if i < len(toks) and isinstance(toks[i], Num):
        return toks[i].v, i + 1
    if _is(toks, i, "("):
        v, i = _expr(toks, i + 1)
        if not _is(toks, i, ")"):
            raise BadSyntax()
        return v, i + 1
    raise BadSyntax()


def calc(s: str) -> int:
    toks = tokenize(s)
    v, i = _expr(toks, 0)
    if i != len(toks):
        raise BadSyntax()
    return v


def show_result(s: str) -> str:
    try:
        return f"{s} = {calc(s)}"
    except BadChar as e:
        return f"{s} = error: bad character {e.ch}"
    except DivByZero:
        return f"{s} = error: division by zero"
    except BadSyntax:
        return f"{s} = error: syntax"


if __name__ == "__main__":
    print(show_result("12 + 30 - 2"))


def test_adds():
    assert calc("1 + 2 + 3") == 6


def test_subtracts():
    assert calc("10 - 4 - 3") == 3


def test_bad_char():
    try:
        calc("2 $ 3")
        assert False
    except BadChar as e:
        assert e.ch == "$"


def test_shown():
    assert show_result("7 - 2") == "7 - 2 = 5"


def test_precedence():
    assert calc("2 + 3 * 4") == 14


def test_unary_parens():
    assert calc("-(2 + 3) * 2") == -10


def test_syntax():
    assert show_result("(1") == "(1 = error: syntax"
