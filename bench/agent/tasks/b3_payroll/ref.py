from dataclasses import dataclass
from typing import List, Tuple


@dataclass(frozen=True)
class Row:
    name: str
    rate: int
    hours: int


class CsvErr(Exception):
    pass


class BadRow(CsvErr):
    def __init__(self, line: int):
        super().__init__(line)
        self.line = line


class BadNumber(CsvErr):
    def __init__(self, line: int, field: str):
        super().__init__(line, field)
        self.line = line
        self.field = field


def sample() -> str:
    return "Beth,400,0\nDan,375,0\nKathy,400,10\nMark,500,20\nMary,550,22\nSusie,425,18"


def _num(s: str, line: int, field: str) -> int:
    try:
        n = int(s)
    except ValueError:
        raise BadNumber(line, field)
    if n < 0:
        raise BadNumber(line, field)
    return n


def parse_row(line: str, n: int) -> Row:
    f = [x.strip() for x in line.split(",")]
    if len(f) != 3:
        raise BadRow(n)
    return Row(f[0], _num(f[1], n, "rate"), _num(f[2], n, "hours"))


def _data_lines(text: str) -> List[Tuple[int, str]]:
    out = []
    for i, line in enumerate(text.split("\n"), 1):
        t = line.strip()
        if not t or (i == 1 and t == "name,rate,hours"):
            continue
        out.append((i, line))
    return out


def parse(text: str) -> List[Row]:
    return [parse_row(line, n) for n, line in _data_lines(text)]


def pay(r: Row) -> int:
    if r.hours > 40:
        return r.rate * 40 + r.rate * 3 * (r.hours - 40) // 2
    return r.rate * r.hours


def report(rows: List[Row]) -> str:
    lines = [f"{r.name}: {pay(r)}" for r in sorted(rows, key=lambda r: (-pay(r), r.name))]
    return "\n".join(lines + [f"total: {sum(pay(r) for r in rows)}"])


def validate(text: str) -> List[str]:
    out = []
    for n, line in _data_lines(text):
        try:
            parse_row(line, n)
        except BadRow as e:
            out.append(f"line {e.line}: expected 3 fields")
        except BadNumber as e:
            out.append(f"line {e.line}: bad {e.field}")
    return out


if __name__ == "__main__":
    print(report(parse(sample())))


def test_parsed():
    assert len(parse(sample())) == 6 and parse(sample())[2] == Row("Kathy", 400, 10)


def test_paid():
    assert pay(Row("Mark", 500, 20)) == 10000


def test_reported():
    assert report(parse("Beth,400,0\nKathy,400,10")) == "Kathy: 4000\nBeth: 0\ntotal: 4000"


def test_overtime():
    assert pay(Row("x", 10, 42)) == 430


def test_validated():
    assert validate("a,1\nb,2,x") == ["line 1: expected 3 fields", "line 2: bad hours"]
