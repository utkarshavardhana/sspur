from dataclasses import dataclass, replace
from typing import List, Optional, Tuple


@dataclass(frozen=True)
class Entry:
    key: str
    value: int


@dataclass(frozen=True)
class Cache:
    cap: int
    entries: List[Entry]
    hits: int = 0
    misses: int = 0


class CacheErr(Exception):
    pass


class BadCapacity(CacheErr):
    def __init__(self, cap: int):
        super().__init__(cap)
        self.cap = cap


def new_cache(cap: int) -> Cache:
    if cap < 1:
        raise BadCapacity(cap)
    return Cache(cap, [])


def _trim(c: Cache) -> Cache:
    return replace(c, entries=c.entries[max(0, len(c.entries) - c.cap):])


def _touch(c: Cache, key: str, value: int) -> Cache:
    return replace(c, entries=[e for e in c.entries if e.key != key] + [Entry(key, value)])


def peek(c: Cache, key: str) -> Optional[int]:
    for e in c.entries:
        if e.key == key:
            return e.value
    return None


def lookup(c: Cache, key: str) -> Tuple[Optional[int], Cache]:
    v = peek(c, key)
    if v is None:
        return None, replace(c, misses=c.misses + 1)
    return v, replace(_touch(c, key, v), hits=c.hits + 1)


def get_or(c: Cache, key: str, default: int) -> Tuple[int, Cache]:
    v, c2 = lookup(c, key)
    return (default if v is None else v), c2


def put(c: Cache, key: str, value: int) -> Cache:
    return _trim(_touch(c, key, value))


def resize(c: Cache, cap: int) -> Cache:
    if cap < 1:
        raise BadCapacity(cap)
    return _trim(replace(c, cap=cap))


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
    assert lookup(put(new_cache(2), "a", 1), "a")[0] == 1


def test_lookup_miss():
    assert get_or(new_cache(2), "z", -1)[0] == -1


def test_described():
    assert describe(put(new_cache(3), "x", 7)) == "cap=3 [x=7]"


def test_evicts():
    assert keys(put(put(put(new_cache(2), "a", 1), "b", 2), "c", 3)) == ["b", "c"]
