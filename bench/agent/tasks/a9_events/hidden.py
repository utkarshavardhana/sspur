import pytest
from app import *


def ev(ts, level, msg):
    return Event(ts, level, msg)


def test_hidden_parse_new():
    assert parse_line("[42] ERROR: disk full") == ev(42, "ERROR", "disk full")


def test_hidden_parse_trim():
    assert parse_line("[7] WARN:    spaced out   ") == ev(7, "WARN", "spaced out")


def test_hidden_parse_empty_msg():
    assert parse_line("[3] INFO: ") == ev(3, "INFO", "")


def test_hidden_parse_old_rejected():
    assert parse_line("5|INFO|up") is None


def test_hidden_parse_bad():
    assert parse_line("[x] INFO: a") is None and parse_line("[1] info: a") is None and parse_line("1] INFO: a") is None and parse_line("[1]INFO: a") is None


def test_hidden_ingest():
    assert ingest(["[1] INFO: a", "junk", "[2] WARN: b"]) == [ev(1, "INFO", "a"), ev(2, "WARN", "b")]


def test_hidden_to_json():
    assert to_json([ev(1, "INFO", "up"), ev(2, "WARN", "x y")]) == '[{"ts":1,"level":"INFO","msg":"up"},{"ts":2,"level":"WARN","msg":"x y"}]'


def test_hidden_to_json_empty():
    assert to_json([]) == "[]"


def test_hidden_from_json():
    assert from_json('[{"ts": 9, "level": "INFO", "msg": "hi"}]') == [ev(9, "INFO", "hi")]


def test_hidden_json_round():
    assert from_json(to_json([ev(1, "A", 'q"uote')])) == [ev(1, "A", 'q"uote')]


def test_hidden_from_json_bad():
    with pytest.raises(BadJson) as e:
        from_json("nope")
    assert e.value.text == "nope"


def test_hidden_from_json_shape():
    with pytest.raises(BadJson) as e:
        from_json('[{"ts": "1", "level": "A", "msg": ""}]')
    assert e.value.text.startswith("[")


def test_hidden_from_json_missing():
    with pytest.raises(BadJson):
        from_json('[{"ts": 1, "level": "A"}]')


def test_hidden_redact():
    assert redact(ev(1, "INFO", "from bob.smith@mail.example.com to a_b-c@x-y.io.")).msg == "from <email> to <email>."


def test_hidden_redact_none():
    assert redact(ev(4, "INFO", "no @ here or a@b")) == ev(4, "INFO", "no @ here or a@b")


def test_hidden_skipped():
    assert skipped(["[1] INFO: a", "junk", "[2] WARN: b", "", "2|INFO|old"]) == ["junk", "", "2|INFO|old"]


def test_hidden_counts():
    assert count_by_level(ingest(["[1] B: x", "[2] A: y", "[3] B: z"])) == [("B", 2), ("A", 1)]
