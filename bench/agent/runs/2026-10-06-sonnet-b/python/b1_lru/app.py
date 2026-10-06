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


def peek(c: Cache, key: str) -> Optional[int]:
    for e in c.entries:
        if e.key == key:
            return e.value
    return None


def lookup(c: Cache, key: str) -> Tuple[Optional[int], Cache]:
    for e in c.entries:
        if e.key == key:
            rest = [x for x in c.entries if x.key != key]
            return e.value, replace(c, entries=rest + [e], hits=c.hits + 1)
    return None, replace(c, misses=c.misses + 1)


def get_or(c: Cache, key: str, default: int) -> Tuple[int, Cache]:
    v, c2 = lookup(c, key)
    return (default if v is None else v), c2


def put(c: Cache, key: str, value: int) -> Cache:
    rest = [e for e in c.entries if e.key != key]
    if len(rest) >= c.cap:
        rest = rest[len(rest) - c.cap + 1:]
    return replace(c, entries=rest + [Entry(key, value)])


def resize(c: Cache, cap: int) -> Cache:
    if cap < 1:
        raise BadCapacity(cap)
    entries = c.entries[max(0, len(c.entries) - cap):]
    return replace(c, cap=cap, entries=entries)


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
    v, c = lookup(put(new_cache(2), "a", 1), "a")
    assert v == 1 and c.hits == 1 and c.misses == 0


def test_lookup_miss():
    v, c = lookup(put(new_cache(2), "a", 1), "z")
    assert v is None and c.misses == 1 and c.hits == 0 and keys(c) == ["a"]


def test_get_or():
    v, c = get_or(new_cache(2), "z", -1)
    assert v == -1 and c.misses == 1
    v, c = get_or(put(c, "z", 5), "z", -1)
    assert v == 5 and c.hits == 1


def test_lookup_recency():
    c = put(put(new_cache(2), "a", 1), "b", 2)
    _, c = lookup(c, "a")
    assert keys(c) == ["b", "a"]
    assert keys(put(c, "c", 3)) == ["a", "c"]


def test_bad_capacity():
    for f in (lambda: new_cache(0), lambda: resize(new_cache(1), 0)):
        try:
            f()
            assert False
        except BadCapacity as e:
            assert e.cap == 0


def test_put_evicts_and_replaces():
    c = put(put(new_cache(2), "a", 1), "b", 2)
    assert keys(put(c, "c", 3)) == ["b", "c"]
    c2 = put(c, "a", 9)
    assert keys(c2) == ["b", "a"] and peek(c2, "a") == 9


def test_peek():
    c = put(put(new_cache(2), "a", 1), "b", 2)
    assert peek(c, "a") == 1 and peek(c, "z") is None
    assert keys(c) == ["a", "b"] and c.hits == 0 and c.misses == 0


def test_resize():
    c = put(put(put(new_cache(3), "a", 1), "b", 2), "c", 3)
    r = resize(c, 2)
    assert r.cap == 2 and keys(r) == ["b", "c"]
    g = resize(c, 5)
    assert g.cap == 5 and keys(g) == ["a", "b", "c"]


def test_described():
    assert describe(put(new_cache(3), "x", 7)) == "cap=3 [x=7]"
