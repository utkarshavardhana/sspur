import pytest
from app import *


def h_res(s):
    try:
        return str(calc(s))
    except DivByZero:
        return "div"
    except BadSyntax:
        return "syntax"
    except BadChar as e:
        return "char " + e.ch
    except CalcErr:
        return "other"


def test_hidden_precedence():
    assert h_res("1 + 2 * 3") == "7" and h_res("2 * 3 + 4") == "10" and h_res("20 - 6 / 3") == "18"


def test_hidden_left_assoc():
    assert h_res("8 - 3 - 2") == "3" and h_res("16 / 4 / 2") == "2" and h_res("2 * 3 / 4") == "1"


def test_hidden_truncate():
    assert h_res("7 / 2") == "3" and h_res("-7 / 2") == "-3" and h_res("7 / -2") == "-3"


def test_hidden_no_spaces():
    assert h_res("12*3-4") == "32"


def test_hidden_div_zero():
    assert h_res("1 / 0") == "div" and h_res("5 / (2 - 2)") == "div"


def test_hidden_parens():
    assert h_res("2 * (3 + 4)") == "14" and h_res("((2))") == "2" and h_res("(1 + 2) * (3 + 4)") == "21" and h_res("10 - (2 - 3)") == "11"


def test_hidden_nested_parens():
    assert h_res("2 * (3 + (4 - 1) * 2)") == "18"


def test_hidden_unary():
    assert h_res("-3") == "-3" and h_res("2 * -3") == "-6" and h_res("-(1 + 2)") == "-3" and h_res("2 - -3") == "5" and h_res("(-4) * 2") == "-8"


def test_hidden_empty():
    assert h_res("") == "syntax" and h_res("   ") == "syntax"


def test_hidden_missing_operand():
    assert h_res("1 +") == "syntax" and h_res("* 2") == "syntax"


def test_hidden_two_numbers():
    assert h_res("1 2") == "syntax"


def test_hidden_unbalanced():
    assert h_res("(1 + 2") == "syntax" and h_res("1 + 2)") == "syntax"


def test_hidden_empty_parens():
    assert h_res("()") == "syntax"


def test_hidden_bad_char():
    assert h_res("1 $ 2") == "char $"


def test_hidden_show_div():
    assert show_result("1 / 0") == "1 / 0 = error: division by zero"


def test_hidden_show_syntax():
    assert show_result("1 +") == "1 + = error: syntax"


def test_hidden_show_ok():
    assert show_result("2 * (3 + 4)") == "2 * (3 + 4) = 14" and show_result("1 ? 2") == "1 ? 2 = error: bad character ?"
