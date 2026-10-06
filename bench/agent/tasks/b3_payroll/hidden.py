import pytest
from app import *


def h_err(text):
    try:
        return f"rows {len(parse(text))}"
    except BadRow as e:
        return f"row {e.line}"
    except BadNumber as e:
        return f"number {e.line} {e.field}"
    except CsvErr:
        return "other"


def test_hidden_sample():
    assert len(parse(sample())) == 6 and pay(parse(sample())[2]) == 4000


def test_hidden_header():
    assert h_err("name,rate,hours\nBeth,400,10") == "rows 1"


def test_hidden_header_trimmed():
    assert h_err("  name,rate,hours  \nBeth,400,10") == "rows 1"


def test_hidden_header_not_first():
    assert h_err("Beth,400,10\nname,rate,hours") == "number 2 rate"


def test_hidden_blank_lines():
    assert h_err("\nBeth,400,10\n\n   \nDan,375,2\n") == "rows 2" and h_err("") == "rows 0"


def test_hidden_trims():
    assert parse(" Beth , 400 , 10 ") == [Row("Beth", 400, 10)]


def test_hidden_bad_row():
    assert h_err("Beth,400\nDan,375,2") == "row 1" and h_err("a,1,2\nb,1,2,3") == "row 2"


def test_hidden_line_numbers():
    assert h_err("name,rate,hours\n\nBeth,4x,1") == "number 3 rate"


def test_hidden_bad_hours():
    assert h_err("Beth,400,ten") == "number 1 hours" and h_err("Beth,400,-5") == "number 1 hours"


def test_hidden_rate_first():
    assert h_err("Beth,x,y") == "number 1 rate" and h_err("Beth,-1,3") == "number 1 rate"


def test_hidden_overtime():
    assert pay(Row("x", 400, 45)) == 19000 and pay(Row("y", 5, 41)) == 207


def test_hidden_no_overtime():
    assert pay(Row("x", 400, 40)) == 16000 and pay(Row("z", 375, 0)) == 0


def test_hidden_report_sorted():
    assert report(parse("Beth,400,0\nDan,375,0\nKathy,400,10")) == "Kathy: 4000\nBeth: 0\nDan: 0\ntotal: 4000"


def test_hidden_report_overtime():
    assert report([Row("a", 10, 50), Row("b", 20, 30)]) == "b: 600\na: 550\ntotal: 1150"


def test_hidden_report_empty():
    assert report([]) == "total: 0"


def test_hidden_validate():
    assert validate("name,rate,hours\nBeth,400\nDan,x,1\nKathy,400,10\nMark,500,z") == ["line 2: expected 3 fields", "line 3: bad rate", "line 5: bad hours"]


def test_hidden_validate_ok():
    assert validate(sample()) == [] and validate("") == []
