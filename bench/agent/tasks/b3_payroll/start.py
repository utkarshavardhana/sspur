from dataclasses import dataclass
from typing import List


@dataclass(frozen=True)
class Row:
    name: str
    rate: int
    hours: int


def sample() -> str:
    return "Beth,400,0\nDan,375,0\nKathy,400,10\nMark,500,20\nMary,550,22\nSusie,425,18"


def _to_int(s: str) -> int:
    try:
        return int(s)
    except ValueError:
        return 0


def parse_row(line: str) -> Row:
    f = line.split(",")
    return Row(f[0], _to_int(f[1]), _to_int(f[2]))


def parse(text: str) -> List[Row]:
    return [parse_row(line) for line in text.split("\n")]


def pay(r: Row) -> int:
    return r.rate * r.hours


def report(rows: List[Row]) -> str:
    return "\n".join(f"{r.name}: {pay(r)}" for r in rows)


if __name__ == "__main__":
    print(report(parse(sample())))


def test_parsed():
    assert len(parse(sample())) == 6 and parse(sample())[2] == Row("Kathy", 400, 10)


def test_paid():
    assert pay(Row("Mark", 500, 20)) == 10000


def test_reported():
    assert report(parse("Beth,400,0\nKathy,400,10")) == "Beth: 0\nKathy: 4000"
