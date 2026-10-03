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


def parse_line(line: str) -> Optional[Entry]:
    parts = line.split("=")
    if len(parts) == 2:
        return Entry(parts[0], parts[1])
    return None


def parse(text: str) -> List[Entry]:
    return [e for e in (parse_line(l) for l in text.split(";")) if e is not None]


def lookup(cfg: List[Entry], key: str) -> Optional[str]:
    return next((e.value for e in cfg if e.key == key), None)


def get_str(cfg: List[Entry], key: str) -> str:
    v = lookup(cfg, key)
    if v is None:
        raise Missing(key)
    return v


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
