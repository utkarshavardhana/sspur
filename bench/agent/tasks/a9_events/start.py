from collections import Counter
from dataclasses import dataclass
from typing import List, Optional, Tuple


@dataclass(frozen=True)
class Event:
    ts: int
    level: str
    msg: str


def log(msg: str) -> None:
    print(msg)


def parse_line(line: str) -> Optional[Event]:
    parts = line.split("|")
    if len(parts) != 3:
        return None
    try:
        ts = int(parts[0].strip())
    except ValueError:
        return None
    return Event(ts, parts[1].strip(), parts[2].strip())


def ingest(lines: List[str]) -> List[Event]:
    out = []
    for l in lines:
        e = parse_line(l)
        if e is None:
            log("skip: " + l)
        else:
            out.append(e)
    return out


def count_by_level(evs: List[Event]) -> List[Tuple[str, int]]:
    return list(Counter(e.level for e in evs).items())


def sample() -> List[str]:
    return ["1|INFO|start", "2|WARN|disk low", "oops", "3|INFO|done"]


def main() -> None:
    log(",".join(f"{k}={v}" for k, v in count_by_level(ingest(sample()))))


def test_parse_ok():
    assert parse_line("5|INFO|up") == Event(5, "INFO", "up")


def test_parse_bad():
    assert parse_line("x|INFO|up") is None


def test_counts():
    assert count_by_level([Event(1, "B", ""), Event(2, "A", ""), Event(3, "B", "")]) == [("B", 2), ("A", 1)]
