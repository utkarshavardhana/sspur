import re
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
    line = line.strip()
    if line.startswith("#") or "=" not in line:
        return None
    k, v = line.split("=", 1)
    k = k.strip()
    if not k:
        return None
    return Entry(k, v.strip())


def parse(text: str) -> List[Entry]:
    return [e for e in (parse_line(l) for l in text.split(";")) if e is not None]


def lookup(cfg: List[Entry], key: str) -> Optional[str]:
    return next((e.value for e in reversed(cfg) if e.key == key), None)


def get_str(cfg: List[Entry], key: str) -> str:
    v = lookup(cfg, key)
    if v is None:
        raise Missing(key)
    return v


_INT_RE = re.compile(r"-?[0-9]+")


def get_int(cfg: List[Entry], key: str) -> int:
    v = get_str(cfg, key)
    if not _INT_RE.fullmatch(v):
        raise NotInt(key, v)
    return int(v)


_TRUE = {"true", "yes", "1"}
_FALSE = {"false", "no", "0"}


def get_bool(cfg: List[Entry], key: str, default: bool) -> bool:
    v = lookup(cfg, key)
    if v is None:
        return default
    lv = v.lower()
    if lv in _TRUE:
        return True
    if lv in _FALSE:
        return False
    raise NotBool(key, v)


def keys(cfg: List[Entry]) -> List[str]:
    return list(dict.fromkeys(e.key for e in cfg))


def sample_text() -> str:
    return "host=localhost;port=8080;debug=true"


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


def test_trim_and_skip():
    cfg = parse("  host = example ; novalue ; = x ;  =  ; port=1")
    assert [(e.key, e.value) for e in cfg] == [("host", "example"), ("port", "1")]


def test_split_first_eq():
    assert lookup(parse("url=a=b"), "url") == "a=b"


def test_comments():
    cfg = parse("# c=1; host=h;  #x=y")
    assert keys(cfg) == ["host"]


def test_last_wins_keys_first_seen():
    cfg = parse("a=1;b=2;a=3")
    assert lookup(cfg, "a") == "3"
    assert get_str(cfg, "a") == "3"
    assert keys(cfg) == ["a", "b"]


def test_get_int():
    cfg = parse("p=8080;n=-5;bad=12a;e=;m=-;plus=+3")
    assert get_int(cfg, "p") == 8080
    assert get_int(cfg, "n") == -5
    for k in ("bad", "e", "m", "plus"):
        try:
            get_int(cfg, k)
            assert False
        except NotInt as e:
            assert e.key == k
            assert e.value == lookup(cfg, k)
    try:
        get_int(cfg, "nope")
        assert False
    except Missing as e:
        assert e.key == "nope"


def test_get_bool():
    cfg = parse("a=TRUE;b=Yes;c=1;d=false;e=NO;f=0;g=maybe")
    assert get_bool(cfg, "a", False) is True
    assert get_bool(cfg, "b", False) is True
    assert get_bool(cfg, "c", False) is True
    assert get_bool(cfg, "d", True) is False
    assert get_bool(cfg, "e", True) is False
    assert get_bool(cfg, "f", True) is False
    assert get_bool(cfg, "zz", True) is True
    assert get_bool(cfg, "zz", False) is False
    try:
        get_bool(cfg, "g", True)
        assert False
    except NotBool as e:
        assert e.key == "g" and e.value == "maybe"
