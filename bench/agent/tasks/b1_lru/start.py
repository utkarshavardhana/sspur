from dataclasses import dataclass, replace
from typing import List, Optional


@dataclass(frozen=True)
class Entry:
    key: str
    value: int


@dataclass(frozen=True)
class Cache:
    cap: int
    entries: List[Entry]


class CacheErr(Exception):
    pass


class BadCapacity(CacheErr):
    def __init__(self, cap: int):
        super().__init__(cap)
        self.cap = cap


def new_cache(cap: int) -> Cache:
    return Cache(cap, [])


def lookup(c: Cache, key: str) -> Optional[int]:
    for e in c.entries:
        if e.key == key:
            return e.value
    return None


def get_or(c: Cache, key: str, default: int) -> int:
    v = lookup(c, key)
    return default if v is None else v


def put(c: Cache, key: str, value: int) -> Cache:
    return replace(c, entries=[e for e in c.entries if e.key != key] + [Entry(key, value)])


def keys(c: Cache) -> List[str]:
    return [e.key for e in c.entries]


def size(c: Cache) -> int:
    return len(c.entries)


def describe(c: Cache) -> str:
    return f"cap={c.cap} [" + ", ".join(f"{e.key}={e.value}" for e in c.entries) + "]"


if __name__ == "__main__":
    print(describe(put(put(new_cache(2), "a", 1), "b", 2)))


def test_put_two():
    assert keys(put(put(new_cache(2), "a", 1), "b", 2)) == ["a", "b"]


def test_lookup_hit():
    assert lookup(put(new_cache(2), "a", 1), "a") == 1


def test_lookup_miss():
    assert get_or(new_cache(2), "z", -1) == -1


def test_described():
    assert describe(put(new_cache(3), "x", 7)) == "cap=3 [x=7]"
