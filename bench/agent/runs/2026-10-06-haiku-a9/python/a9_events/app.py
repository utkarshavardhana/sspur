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
    def __init__(self, text: str):
        self.text = text
        super().__init__(f"BadJson: {text}")


def log(msg: str) -> None:
    print(msg)


def parse_line(line: str) -> Optional[Event]:
    match = re.match(r'^\[(\d+)\]\s+([A-Z]+):\s*(.*?)\s*$', line)
    if not match:
        return None
    ts = int(match.group(1))
    level = match.group(2)
    msg = match.group(3)
    return Event(ts, level, msg)


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
    data = [{"ts": e.ts, "level": e.level, "msg": e.msg} for e in evs]
    return json.dumps(data, separators=(',', ':'))


def from_json(s: str) -> List[Event]:
    try:
        data = json.loads(s)
    except json.JSONDecodeError:
        raise BadJson(s)

    if not isinstance(data, list):
        raise BadJson(s)

    events = []
    for item in data:
        if not isinstance(item, dict):
            raise BadJson(s)
        if "ts" not in item or "level" not in item or "msg" not in item:
            raise BadJson(s)
        if not isinstance(item["ts"], int) or not isinstance(item["level"], str) or not isinstance(item["msg"], str):
            raise BadJson(s)
        events.append(Event(item["ts"], item["level"], item["msg"]))

    return events


def redact(e: Event) -> Event:
    email_pattern = r'[A-Za-z0-9._+-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+'
    redacted_msg = re.sub(email_pattern, '<email>', e.msg)
    return Event(e.ts, e.level, redacted_msg)


_skipped_lines: List[str] = []


def skipped(lines: List[str]) -> List[str]:
    global _skipped_lines
    _skipped_lines = []
    for l in lines:
        if parse_line(l) is None:
            _skipped_lines.append(l)
    return _skipped_lines


def sample() -> List[str]:
    return ["[1] INFO: start", "[2] WARN: disk low", "oops", "[3] INFO: done"]


def main() -> None:
    log(",".join(f"{k}={v}" for k, v in count_by_level(ingest(sample()))))


def test_parse_ok():
    assert parse_line("[5] INFO: up") == Event(5, "INFO", "up")


def test_parse_with_spaces():
    assert parse_line("[5]  INFO:  up") == Event(5, "INFO", "up")


def test_parse_empty_msg():
    assert parse_line("[5] INFO:") == Event(5, "INFO", "")


def test_parse_bad():
    assert parse_line("5|INFO|up") is None
    assert parse_line("x|INFO|up") is None
    assert parse_line("[x] INFO: up") is None
    assert parse_line("[5] info: up") is None


def test_counts():
    assert count_by_level([Event(1, "B", ""), Event(2, "A", ""), Event(3, "B", "")]) == [("B", 2), ("A", 1)]


def test_to_json():
    evs = [Event(1, "INFO", "up"), Event(2, "WARN", "disk")]
    json_str = to_json(evs)
    assert json_str == '[{"ts":1,"level":"INFO","msg":"up"},{"ts":2,"level":"WARN","msg":"disk"}]'


def test_from_json_ok():
    json_str = '[{"ts":1,"level":"INFO","msg":"up"}]'
    evs = from_json(json_str)
    assert evs == [Event(1, "INFO", "up")]


def test_from_json_multiple():
    json_str = '[{"ts":1,"level":"INFO","msg":"up"},{"ts":2,"level":"WARN","msg":"disk"}]'
    evs = from_json(json_str)
    assert evs == [Event(1, "INFO", "up"), Event(2, "WARN", "disk")]


def test_from_json_roundtrip():
    evs = [Event(1, "INFO", "up"), Event(2, "WARN", "disk")]
    json_str = to_json(evs)
    restored = from_json(json_str)
    assert restored == evs


def test_from_json_bad_json():
    try:
        from_json("{invalid}")
        assert False, "Should raise BadJson"
    except BadJson as e:
        assert e.text == "{invalid}"


def test_from_json_not_list():
    try:
        from_json('{"ts":1,"level":"INFO","msg":"up"}')
        assert False, "Should raise BadJson"
    except BadJson:
        pass


def test_from_json_missing_field():
    try:
        from_json('[{"ts":1,"level":"INFO"}]')
        assert False, "Should raise BadJson"
    except BadJson:
        pass


def test_from_json_wrong_type():
    try:
        from_json('[{"ts":"1","level":"INFO","msg":"up"}]')
        assert False, "Should raise BadJson"
    except BadJson:
        pass


def test_redact():
    e = Event(1, "INFO", "user@example.com")
    redacted = redact(e)
    assert redacted == Event(1, "INFO", "<email>")


def test_redact_multiple_emails():
    e = Event(1, "INFO", "email1@example.com and email2@test.org")
    redacted = redact(e)
    assert redacted == Event(1, "INFO", "<email> and <email>")


def test_redact_complex_email():
    e = Event(1, "INFO", "user.name+tag@sub.example.com")
    redacted = redact(e)
    assert redacted == Event(1, "INFO", "<email>")


def test_redact_no_email():
    e = Event(1, "INFO", "hello world")
    redacted = redact(e)
    assert redacted == Event(1, "INFO", "hello world")


def test_skipped():
    lines = ["[1] INFO: start", "oops", "[2] WARN: disk", "invalid"]
    result = skipped(lines)
    assert result == ["oops", "invalid"]


def test_skipped_all_valid():
    lines = ["[1] INFO: start", "[2] WARN: disk"]
    result = skipped(lines)
    assert result == []
