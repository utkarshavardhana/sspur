from dataclasses import dataclass
from typing import List


@dataclass(frozen=True)
class Row:
    name: str
    rate: int
    hours: int


def sample() -> str:
    return "Beth,400,0\nDan,375,0\nKathy,400,10\nMark,500,20\nMary,550,22\nSusie,425,18"


class CsvErr(Exception):
    pass


class BadRow(CsvErr):
    def __init__(self, line: int):
        super().__init__(f"line {line}: expected 3 fields")
        self.line = line


class BadNumber(CsvErr):
    def __init__(self, line: int, field: str):
        super().__init__(f"line {line}: bad {field}")
        self.line = line
        self.field = field


def _is_num(s: str) -> bool:
    return s != "" and s.isascii() and s.isdigit()


def _rows(text: str):
    for i, raw in enumerate(text.split("\n"), 1):
        line = raw.strip()
        if line == "" or (i == 1 and line == "name,rate,hours"):
            continue
        yield i, [f.strip() for f in line.split(",")]


def _check(i: int, f: List[str]) -> None:
    if len(f) != 3:
        raise BadRow(i)
    if not _is_num(f[1]):
        raise BadNumber(i, "rate")
    if not _is_num(f[2]):
        raise BadNumber(i, "hours")


def parse(text: str) -> List[Row]:
    out = []
    for i, f in _rows(text):
        _check(i, f)
        out.append(Row(f[0], int(f[1]), int(f[2])))
    return out


def validate(text: str) -> List[str]:
    msgs = []
    for i, f in _rows(text):
        try:
            _check(i, f)
        except CsvErr as e:
            msgs.append(str(e))
    return msgs


def pay(r: Row) -> int:
    p = r.rate * min(r.hours, 40)
    if r.hours > 40:
        p += r.rate * 3 * (r.hours - 40) // 2
    return p


def report(rows: List[Row]) -> str:
    ordered = sorted(rows, key=lambda r: (-pay(r), r.name))
    lines = [f"{r.name}: {pay(r)}" for r in ordered]
    lines.append(f"total: {sum(pay(r) for r in rows)}")
    return "\n".join(lines)


if __name__ == "__main__":
    print(report(parse(sample())))


def test_parsed():
    assert len(parse(sample())) == 6 and parse(sample())[2] == Row("Kathy", 400, 10)


def test_paid():
    assert pay(Row("Mark", 500, 20)) == 10000


def test_reported():
    assert report(parse("Beth,400,0\nKathy,400,10")) == "Kathy: 4000\nBeth: 0\ntotal: 4000"


def test_header_blank_trim():
    rows = parse(" name,rate,hours \n\n a , 5 , 2 \n  \nb,1,1")
    assert rows == [Row("a", 5, 2), Row("b", 1, 1)]
    assert parse("") == []


def test_header_only_first_line():
    import pytest
    with pytest.raises(BadNumber):
        parse("a,1,1\nname,rate,hours")


def test_bad_row_line():
    import pytest
    with pytest.raises(BadRow) as e:
        parse("name,rate,hours\n\na,1,1\na,1")
    assert e.value.line == 4


def test_bad_number():
    import pytest
    for text, line, field in [("a,x,1", 1, "rate"), ("a,1,-2", 1, "hours"),
                              ("\na,1,1\na,x,y", 3, "rate"), ("a,1.5,1", 1, "rate"),
                              ("a,1,", 1, "hours")]:
        with pytest.raises(BadNumber) as e:
            parse(text)
        assert (e.value.line, e.value.field) == (line, field)


def test_overtime():
    assert pay(Row("a", 10, 40)) == 400
    assert pay(Row("a", 10, 41)) == 415
    assert pay(Row("a", 5, 45)) == 200 + 37


def test_report_sorted_total():
    rows = [Row("b", 10, 1), Row("a", 10, 1), Row("c", 20, 1)]
    assert report(rows) == "c: 20\na: 10\nb: 10\ntotal: 40"
    assert report([]) == "total: 0"


def test_validate():
    assert validate(sample()) == []
    text = "name,rate,hours\na,1\nb,x,1\n\nc,1,1\nd,1,y"
    assert validate(text) == ["line 2: expected 3 fields", "line 3: bad rate", "line 6: bad hours"]
