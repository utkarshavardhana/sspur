from dataclasses import dataclass
from typing import List, Union


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
            if ch == "+" or ch == "-":
                toks.append(Op(ch))
            elif ch != " ":
                raise BadChar(ch)
    if num:
        toks.append(Num(int(num)))
    return toks


def eval_tokens(toks: List[Tok]) -> int:
    total = 0
    sign = 1
    for t in toks:
        if isinstance(t, Num):
            total += sign * t.v
        else:
            sign = -1 if t.c == "-" else 1
    return total


def calc(s: str) -> int:
    return eval_tokens(tokenize(s))


def show_result(s: str) -> str:
    try:
        return f"{s} = {calc(s)}"
    except BadChar as e:
        return f"{s} = error: bad character {e.ch}"


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
