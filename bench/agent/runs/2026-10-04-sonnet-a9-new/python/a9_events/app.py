import json
import re
from collections import Counter
from dataclasses import dataclass, replace
from typing import List, Optional, Tuple

import pytest


@dataclass(frozen=True)
class Event:
    ts: int
    level: str
    msg: str


def log(msg: str) -> None:
    print(msg)


_LINE_RE = re.compile(r"\[([0-9]+)\] ([A-Z]+):(.*)", re.DOTALL)
_EMAIL_RE = re.compile(r"[A-Za-z0-9._-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+")


def parse_line(line: str) -> Optional[Event]:
    m = _LINE_RE.fullmatch(line)
    if m is None:
        return None
    return Event(int(m.group(1)), m.group(2), m.group(3).strip())


class EventErr(Exception):
    pass


class BadJson(EventErr):
    def __init__(self, text: str):
        super().__init__(text)
        self.text = text


def to_json(evs: List[Event]) -> str:
    return json.dumps(
        [{"ts": e.ts, "level": e.level, "msg": e.msg} for e in evs],
        separators=(",", ":"),
    )


def from_json(s: str) -> List[Event]:
    try:
        data = json.loads(s)
    except (ValueError, RecursionError):
        raise BadJson(s)
    if not isinstance(data, list):
        raise BadJson(s)
    out = []
    for d in data:
        if not isinstance(d, dict):
            raise BadJson(s)
        ts, level, msg = d.get("ts"), d.get("level"), d.get("msg")
        if type(ts) is not int or not isinstance(level, str) or not isinstance(msg, str):
            raise BadJson(s)
        out.append(Event(ts, level, msg))
    return out


def redact(e: Event) -> Event:
    return replace(e, msg=_EMAIL_RE.sub("<email>", e.msg))


def skipped(lines: List[str]) -> List[str]:
    return [l for l in lines if parse_line(l) is None]


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
    return ["[1] INFO: start", "[2] WARN: disk low", "oops", "[3] INFO: done"]


def main() -> None:
    log(",".join(f"{k}={v}" for k, v in count_by_level(ingest(sample()))))


def test_parse_ok():
    assert parse_line("[5] INFO: up") == Event(5, "INFO", "up")
    assert parse_line("[5] INFO:   up  ") == Event(5, "INFO", "up")
    assert parse_line("[5] INFO:") == Event(5, "INFO", "")


def test_parse_bad():
    assert parse_line("5|INFO|up") is None
    assert parse_line("[x] INFO: up") is None
    assert parse_line("[5] info: up") is None
    assert parse_line("[5] INFO up") is None
    assert parse_line("[] INFO: up") is None
    assert parse_line("oops") is None


def test_json_roundtrip():
    evs = [Event(1, "INFO", "up"), Event(2, "WARN", 'a "b"')]
    assert to_json(evs[:1]) == '[{"ts":1,"level":"INFO","msg":"up"}]'
    assert from_json(to_json(evs)) == evs
    assert from_json("[]") == []


def test_from_json_bad():
    for bad in ["nope", "{}", "[1]", '[{"ts":"1","level":"A","msg":""}]',
                '[{"ts":1,"level":"A"}]', '[{"ts":true,"level":"A","msg":""}]',
                '[{"ts":1,"level":1,"msg":""}]']:
        with pytest.raises(BadJson) as ei:
            from_json(bad)
        assert ei.value.text == bad
        assert isinstance(ei.value, EventErr)


def test_redact():
    e = Event(1, "INFO", "mail a.b-c@ex-ample.co.uk and x@y.z now; bad@host")
    assert redact(e) == Event(1, "INFO", "mail <email> and <email> now; bad@host")


def test_skipped():
    assert skipped(sample()) == ["oops"]
    assert len(ingest(sample())) == 3


def test_counts():
    assert count_by_level([Event(1, "B", ""), Event(2, "A", ""), Event(3, "B", "")]) == [("B", 2), ("A", 1)]
