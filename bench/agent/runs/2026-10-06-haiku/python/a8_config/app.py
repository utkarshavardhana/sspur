from dataclasses import dataclass
from typing import List, Optional


@dataclass(frozen=True)
class Entry:
    key: str
    value: str


class CfgErr(Exception):
    pass


class Missing(CfgErr):
    def __init__(self, key: str):
        super().__init__(key)
        self.key = key


class NotInt(CfgErr):
    def __init__(self, key: str, value: str):
        super().__init__(key, value)
        self.key = key
        self.value = value


class NotBool(CfgErr):
    def __init__(self, key: str, value: str):
        super().__init__(key, value)
        self.key = key
        self.value = value


def parse_line(line: str) -> Optional[Entry]:
    trimmed = line.strip()
    if not trimmed or trimmed.startswith("#"):
        return None
    if "=" not in trimmed:
        return None
    key, value = trimmed.split("=", 1)
    key = key.strip()
    if not key:
        return None
    value = value.strip()
    return Entry(key, value)


def parse(text: str) -> List[Entry]:
    return [e for e in (parse_line(l) for l in text.split("\n")) if e is not None]


def lookup(cfg: List[Entry], key: str) -> Optional[str]:
    matching = [e.value for e in cfg if e.key == key]
    return matching[-1] if matching else None


def get_str(cfg: List[Entry], key: str) -> str:
    v = lookup(cfg, key)
    if v is None:
        raise Missing(key)
    return v


def get_int(cfg: List[Entry], key: str) -> int:
    v = lookup(cfg, key)
    if v is None:
        raise Missing(key)
    try:
        return int(v)
    except ValueError:
        raise NotInt(key, v)


def get_bool(cfg: List[Entry], key: str, default: bool) -> bool:
    v = lookup(cfg, key)
    if v is None:
        return default
    v_lower = v.lower()
    if v_lower in ("true", "yes", "1"):
        return True
    if v_lower in ("false", "no", "0"):
        return False
    raise NotBool(key, v)


def keys(cfg: List[Entry]) -> List[str]:
    return list(dict.fromkeys(e.key for e in cfg))


def sample_text() -> str:
    return "host=localhost\nport=8080\ndebug=true"


if __name__ == "__main__":
    print(",".join(keys(parse(sample_text()))))


def test_parse_sample():
    assert len(parse(sample_text())) == 3


def test_lookup_host():
    assert lookup(parse(sample_text()), "host") == "localhost"


def test_missing():
    try:
        get_str(parse(sample_text()), "nope")
        assert False
    except Missing as e:
        assert e.key == "nope"


# Tests for hardening
def test_trim_whitespace():
    text = "  key1  =  value1  \nkey2= value2\n= value3"
    cfg = parse(text)
    assert len(cfg) == 2
    assert lookup(cfg, "key1") == "value1"
    assert lookup(cfg, "key2") == "value2"


def test_no_equals_sign():
    text = "key1=value1\nkey2_no_equals\nkey3=value3"
    cfg = parse(text)
    assert len(cfg) == 2
    assert lookup(cfg, "key1") == "value1"
    assert lookup(cfg, "key3") == "value3"


def test_empty_key_after_trim():
    text = "key1=value1\n   =value2\nkey3=value3"
    cfg = parse(text)
    assert len(cfg) == 2
    assert lookup(cfg, "key1") == "value1"
    assert lookup(cfg, "key3") == "value3"


def test_first_equals_only():
    text = "url=a=b=c\nkey=value=extra=data"
    cfg = parse(text)
    assert lookup(cfg, "url") == "a=b=c"
    assert lookup(cfg, "key") == "value=extra=data"


def test_comments():
    text = "# This is a comment\nkey1=value1\n  # Another comment\nkey2=value2"
    cfg = parse(text)
    assert len(cfg) == 2
    assert lookup(cfg, "key1") == "value1"
    assert lookup(cfg, "key2") == "value2"


def test_override():
    text = "key=first\nkey=second\nkey=third"
    cfg = parse(text)
    assert lookup(cfg, "key") == "third"


def test_get_int_valid():
    cfg = parse("port=8080\nnegative=-42")
    assert get_int(cfg, "port") == 8080
    assert get_int(cfg, "negative") == -42


def test_get_int_missing():
    cfg = parse("port=8080")
    try:
        get_int(cfg, "missing")
        assert False
    except Missing as e:
        assert e.key == "missing"


def test_get_int_not_int():
    cfg = parse("value=not_an_int")
    try:
        get_int(cfg, "value")
        assert False
    except NotInt as e:
        assert e.key == "value"
        assert e.value == "not_an_int"


def test_get_bool_true_variants():
    cfg = parse("a=true\nb=yes\nc=1\nd=TRUE\ne=YES\nf=TrUe")
    assert get_bool(cfg, "a", False) is True
    assert get_bool(cfg, "b", False) is True
    assert get_bool(cfg, "c", False) is True
    assert get_bool(cfg, "d", False) is True
    assert get_bool(cfg, "e", False) is True
    assert get_bool(cfg, "f", False) is True


def test_get_bool_false_variants():
    cfg = parse("a=false\nb=no\nc=0\nd=FALSE\ne=NO\nf=FaLsE")
    assert get_bool(cfg, "a", True) is False
    assert get_bool(cfg, "b", True) is False
    assert get_bool(cfg, "c", True) is False
    assert get_bool(cfg, "d", True) is False
    assert get_bool(cfg, "e", True) is False
    assert get_bool(cfg, "f", True) is False


def test_get_bool_default():
    cfg = parse("other=value")
    assert get_bool(cfg, "missing", True) is True
    assert get_bool(cfg, "missing", False) is False


def test_get_bool_invalid():
    cfg = parse("value=maybe")
    try:
        get_bool(cfg, "value", False)
        assert False
    except NotBool as e:
        assert e.key == "value"
        assert e.value == "maybe"
