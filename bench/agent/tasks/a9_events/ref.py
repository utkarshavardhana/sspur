import json
import re
from collections import Counter
from dataclasses import asdict, dataclass
from typing import List, Optional, Tuple


@dataclass(frozen=True)
class Event:
    ts: int
    level: str
    msg: str


class EventErr(Exception):
    pass


class BadJson(EventErr):
    def __init__(self, text: str):
        super().__init__(text)
        self.text = text


def log(msg: str) -> None:
    print(msg)


LINE = re.compile(r"\[(\d+)\] ([A-Z]+):(.*)")
EMAIL = re.compile(r"[A-Za-z0-9._-]+@[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)+")


def parse_line(line: str) -> Optional[Event]:
    m = LINE.fullmatch(line)
    if not m:
        return None
    return Event(int(m.group(1)), m.group(2), m.group(3).strip())


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


def to_json(evs: List[Event]) -> str:
    return json.dumps([asdict(e) for e in evs], separators=(",", ":"))


def from_json(s: str) -> List[Event]:
    try:
        data = json.loads(s)
        out = []
        for d in data:
            ts, level, msg = d["ts"], d["level"], d["msg"]
            if type(ts) is not int or not isinstance(level, str) or not isinstance(msg, str):
                raise ValueError
            out.append(Event(ts, level, msg))
        return out
    except (ValueError, KeyError, TypeError):
        raise BadJson(s)


def redact(e: Event) -> Event:
    return Event(e.ts, e.level, EMAIL.sub("<email>", e.msg))


def skipped(lines: List[str]) -> List[str]:
    return [l for l in lines if parse_line(l) is None]


def sample() -> List[str]:
    return ["[1] INFO: start", "[2] WARN: disk low", "oops", "[3] INFO: done"]


def main() -> None:
    log(",".join(f"{k}={v}" for k, v in count_by_level(ingest(sample()))))


def test_parse_ok():
    assert parse_line("[5] INFO: up") == Event(5, "INFO", "up")


def test_parse_bad():
    assert parse_line("5|INFO|up") is None


def test_counts():
    assert count_by_level([Event(1, "B", ""), Event(2, "A", ""), Event(3, "B", "")]) == [("B", 2), ("A", 1)]


def test_json_round():
    assert from_json(to_json(ingest(sample()))) == ingest(sample())


def test_redact_one():
    assert redact(Event(1, "I", "mail a.b@x.io now")).msg == "mail <email> now"


def test_skipped_one():
    assert skipped(sample()) == ["oops"]
