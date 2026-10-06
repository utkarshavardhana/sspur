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


class DivByZero(CalcErr):
    pass


class BadSyntax(CalcErr):
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
            if ch in "+-*/()":
                toks.append(Op(ch))
            elif ch != " ":
                raise BadChar(ch)
    if num:
        toks.append(Num(int(num)))
    return toks


def eval_tokens(toks: List[Tok]) -> int:
    pos = 0

    def peek():
        return toks[pos] if pos < len(toks) else None

    def is_op(t, chars: str) -> bool:
        return isinstance(t, Op) and t.c in chars

    def expr() -> int:
        nonlocal pos
        val = term()
        while is_op(peek(), "+-"):
            op = toks[pos].c
            pos += 1
            rhs = term()
            val = val + rhs if op == "+" else val - rhs
        return val

    def term() -> int:
        nonlocal pos
        val = unary()
        while is_op(peek(), "*/"):
            op = toks[pos].c
            pos += 1
            rhs = unary()
            if op == "*":
                val = val * rhs
            else:
                if rhs == 0:
                    raise DivByZero()
                q = abs(val) // abs(rhs)
                val = q if (val < 0) == (rhs < 0) else -q
        return val

    def unary() -> int:
        nonlocal pos
        t = peek()
        if is_op(t, "-"):
            pos += 1
            return -unary()
        return atom()

    def atom() -> int:
        nonlocal pos
        t = peek()
        if isinstance(t, Num):
            pos += 1
            return t.v
        if is_op(t, "("):
            pos += 1
            val = expr()
            if not is_op(peek(), ")"):
                raise BadSyntax()
            pos += 1
            return val
        raise BadSyntax()

    val = expr()
    if pos != len(toks):
        raise BadSyntax()
    return val


def calc(s: str) -> int:
    return eval_tokens(tokenize(s))


def show_result(s: str) -> str:
    try:
        return f"{s} = {calc(s)}"
    except DivByZero:
        return f"{s} = error: division by zero"
    except BadSyntax:
        return f"{s} = error: syntax"
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
    except DivByZero:
        return f"{s} = error: division by zero"
    except BadSyntax:
        return f"{s} = error: syntax"
    except BadChar as e:
        assert e.ch == "$"


def test_shown():
    assert show_result("7 - 2") == "7 - 2 = 5"


def raises(exc, s):
    try:
        calc(s)
    except exc:
        return True
    return False


def test_precedence():
    assert calc("2 + 3 * 4") == 14
    assert calc("2 * 3 + 4") == 10
    assert calc("10 - 2 * 3") == 4


def test_left_assoc_and_div():
    assert calc("8 / 2 / 2") == 2
    assert calc("10 - 3 - 2") == 5
    assert calc("7 / 2") == 3
    assert calc("-7 / 2") == -3
    assert calc("7 / -2") == -3
    assert calc("-7 / -2") == 3


def test_div_by_zero():
    assert raises(DivByZero, "1 / 0")
    assert raises(DivByZero, "1 / (2 - 2)")


def test_parens():
    assert calc("(1 + 2) * 3") == 9
    assert calc("((4))") == 4


def test_unary_minus():
    assert calc("-3") == -3
    assert calc("2 * -3") == -6
    assert calc("-(1 + 2)") == -3
    assert calc("(-3)") == -3
    assert calc("2 - -3") == 5


def test_syntax_errors():
    for s in ["", "   ", "1 +", "* 2", "1 2", "(1 + 2", "1 + 2)", "()", "-", "1 + * 2"]:
        assert raises(BadSyntax, s), s


def test_bad_char_still():
    assert raises(BadChar, "1 % 2")


def test_show_errors():
    assert show_result("1 / 0") == "1 / 0 = error: division by zero"
    assert show_result("1 +") == "1 + = error: syntax"
