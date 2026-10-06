import json
import re
from collections import Counter
from dataclasses import dataclass
from typing import List, Optional, Tuple


@dataclass(frozen=True)
class Event:
    ts: int
    level: str
    msg: str


class EventErr(Exception):
    pass


class BadJson(EventErr):
    def __init__(self, text: str) -> None:
        super().__init__(text)
        self.text = text


_LINE_RE = re.compile(r"\[([0-9]+)\] ([A-Z]+):(.*)", re.DOTALL)
_EMAIL_RE = re.compile(r"[A-Za-z0-9._-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+")


def log(msg: str) -> None:
    print(msg)


def parse_line(line: str) -> Optional[Event]:
    m = _LINE_RE.fullmatch(line)
    if m is None:
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


def skipped(lines: List[str]) -> List[str]:
    return [l for l in lines if parse_line(l) is None]


def to_json(evs: List[Event]) -> str:
    return json.dumps(
        [{"ts": e.ts, "level": e.level, "msg": e.msg} for e in evs],
        separators=(",", ":"),
        ensure_ascii=False,
    )


def from_json(s: str) -> List[Event]:
    try:
        data = json.loads(s)
    except Exception:
        raise BadJson(s)
    if not isinstance(data, list):
        raise BadJson(s)
    out = []
    for o in data:
        if not isinstance(o, dict):
            raise BadJson(s)
        ts, level, msg = o.get("ts"), o.get("level"), o.get("msg")
        if type(ts) is not int or not isinstance(level, str) or not isinstance(msg, str):
            raise BadJson(s)
        out.append(Event(ts, level, msg))
    return out


def redact(e: Event) -> Event:
    return Event(e.ts, e.level, _EMAIL_RE.sub("<email>", e.msg))


def count_by_level(evs: List[Event]) -> List[Tuple[str, int]]:
    return list(Counter(e.level for e in evs).items())


def sample() -> List[str]:
    return ["[1] INFO: start", "[2] WARN: disk low", "oops", "[3] INFO: done"]


def main() -> None:
    log(",".join(f"{k}={v}" for k, v in count_by_level(ingest(sample()))))


def test_parse_ok():
    assert parse_line("[5] INFO: up") == Event(5, "INFO", "up")


def test_parse_trims_and_empty():
    assert parse_line("[7] ERROR:   a b  ") == Event(7, "ERROR", "a b")
    assert parse_line("[7] ERROR:") == Event(7, "ERROR", "")


def test_parse_bad():
    assert parse_line("x|INFO|up") is None
    assert parse_line("5|INFO|up") is None
    assert parse_line("[x] INFO: up") is None
    assert parse_line("[5] info: up") is None
    assert parse_line("[] INFO: up") is None
    assert parse_line("[5] INFO up") is None
    assert parse_line(" [5] INFO: up") is None


def test_counts():
    assert count_by_level([Event(1, "B", ""), Event(2, "A", ""), Event(3, "B", "")]) == [("B", 2), ("A", 1)]


def test_ingest_sample():
    assert count_by_level(ingest(sample())) == [("INFO", 2), ("WARN", 1)]


def test_to_json():
    assert to_json([Event(1, "INFO", "up")]) == '[{"ts":1,"level":"INFO","msg":"up"}]'
    assert to_json([]) == "[]"


def test_from_json_roundtrip():
    evs = [Event(1, "INFO", "up"), Event(2, "WARN", 'q"x')]
    assert from_json(to_json(evs)) == evs


def test_from_json_bad():
    bad = [
        "nope",
        "{}",
        "[1]",
        '[{"ts":1,"level":"I"}]',
        '[{"ts":"1","level":"I","msg":""}]',
        '[{"ts":true,"level":"I","msg":""}]',
        '[{"ts":1.5,"level":"I","msg":""}]',
        '[{"ts":1,"level":2,"msg":""}]',
    ]
    for s in bad:
        try:
            from_json(s)
            assert False, s
        except BadJson as e:
            assert e.text == s
            assert isinstance(e, EventErr)


def test_redact():
    e = Event(1, "INFO", "mail a.b-c_d@ex-1.co.uk and x@y.z now; not a@b or @c.d")
    assert redact(e) == Event(1, "INFO", "mail <email> and <email> now; not a@b or @c.d")


def test_skipped():
    assert skipped(sample()) == ["oops"]
    assert skipped(["1|INFO|x", "[1] INFO: x"]) == ["1|INFO|x"]
