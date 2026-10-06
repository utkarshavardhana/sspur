import math
from dataclasses import dataclass, replace
from typing import List, Tuple

import pytest


class LimitErr(Exception):
    pass


class ClockSkew(LimitErr):
    def __init__(self, last: int, now: int):
        super().__init__(f"clock went backwards: last={last} now={now}")
        self.last = last
        self.now = now


class TooLarge(LimitErr):
    def __init__(self, n: int, capacity: int):
        super().__init__(f"requested {n} exceeds capacity {capacity}")
        self.n = n
        self.capacity = capacity


@dataclass(frozen=True)
class Window:
    limit: int
    size: int
    start: int
    count: int


def new_window(limit: int, size: int) -> Window:
    return Window(limit, size, 0, 0)


def allow(w: Window, now: int) -> Tuple[bool, Window]:
    if now < w.start:
        raise ClockSkew(w.start, now)
    w2 = replace(w, start=now, count=0) if now >= w.start + w.size else w
    if w2.count < w2.limit:
        return True, replace(w2, count=w2.count + 1)
    return False, w2


def run_requests(w: Window, times: List[int]) -> List[bool]:
    cur = w
    out = []
    for t in times:
        ok, cur = allow(cur, t)
        out.append(ok)
    return out


@dataclass(frozen=True)
class Bucket:
    capacity: int
    rate: int
    tokens: int
    last: int


def new_bucket(capacity: int, rate: int, now: int) -> Bucket:
    return Bucket(capacity, rate, capacity, now)


def _refill(b: Bucket, now: int) -> Bucket:
    if now < b.last:
        raise ClockSkew(b.last, now)
    return replace(b, tokens=min(b.capacity, b.tokens + (now - b.last) * b.rate), last=now)


def take(b: Bucket, now: int, n: int) -> Tuple[bool, Bucket]:
    r = _refill(b, now)
    if r.tokens >= n:
        return True, replace(r, tokens=r.tokens - n)
    return False, r


def retry_after(b: Bucket, now: int, n: int) -> int:
    r = _refill(b, now)
    if n > b.capacity:
        raise TooLarge(n, b.capacity)
    if r.tokens >= n:
        return 0
    return math.ceil((n - r.tokens) / b.rate)


@dataclass(frozen=True)
class Keyed:
    capacity: int
    rate: int
    buckets: list


def new_keyed(capacity: int, rate: int) -> Keyed:
    return Keyed(capacity, rate, [])


def take_key(k: Keyed, key, now: int) -> Tuple[bool, Keyed]:
    existing = [bb for kk, bb in k.buckets if kk == key]
    b = existing[0] if existing else new_bucket(k.capacity, k.rate, now)
    ok, nb = take(b, now, 1)
    if existing:
        out = [(kk, nb if kk == key else bb) for kk, bb in k.buckets]
    else:
        out = list(k.buckets) + [(key, nb)]
    return ok, replace(k, buckets=out)


if __name__ == "__main__":
    print(" ".join(str(b).lower() for b in run_requests(new_window(3, 10), [0, 1, 2, 3, 12])))


def test_first_allowed():
    assert allow(new_window(3, 10), 0)[0]


def test_counts():
    assert allow(new_window(3, 10), 0)[1].count == 1


def test_new_window_resets():
    assert run_requests(new_window(1, 10), [0, 10])[-1] is True


def test_window_limit_exact():
    assert run_requests(new_window(3, 10), [0, 1, 2, 3, 4]) == [True, True, True, False, False]


def test_window_skew():
    _, w = allow(new_window(3, 10), 15)
    with pytest.raises(ClockSkew) as e:
        allow(w, 4)
    assert (e.value.last, e.value.now) == (15, 4)


def test_new_bucket_full():
    b = new_bucket(5, 1, 7)
    assert (b.tokens, b.last) == (5, 7)


def test_take_and_refill():
    b = new_bucket(5, 1, 0)
    ok, b = take(b, 0, 5)
    assert ok and b.tokens == 0
    ok, b2 = take(b, 2, 3)
    assert not ok and b2.tokens == 2 and b2.last == 2
    ok, b3 = take(b2, 3, 3)
    assert ok and b3.tokens == 0
    ok, b4 = take(b3, 100, 1)
    assert ok and b4.tokens == 4


def test_take_skew():
    with pytest.raises(ClockSkew) as e:
        take(new_bucket(5, 1, 10), 9, 1)
    assert (e.value.last, e.value.now) == (10, 9)


def test_retry_after():
    b = new_bucket(10, 2, 0)
    _, b = take(b, 0, 10)
    assert retry_after(b, 0, 5) == 3
    assert retry_after(b, 1, 2) == 0
    assert retry_after(b, 0, 1) == 1
    with pytest.raises(TooLarge) as e:
        retry_after(b, 0, 11)
    assert (e.value.n, e.value.capacity) == (11, 10)
    with pytest.raises(ClockSkew):
        retry_after(new_bucket(10, 2, 5), 4, 1)


def test_keyed():
    k = new_keyed(1, 1)
    assert k.buckets == []
    ok, k = take_key(k, "a", 0)
    assert ok
    ok, k = take_key(k, "a", 0)
    assert not ok
    ok, k = take_key(k, "b", 0)
    assert ok
    assert len(k.buckets) == 2
    ok, k = take_key(k, "a", 1)
    assert ok
