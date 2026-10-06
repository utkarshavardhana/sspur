from dataclasses import dataclass, replace
from typing import List, Tuple


@dataclass(frozen=True)
class Window:
    limit: int
    size: int
    start: int
    count: int


@dataclass(frozen=True)
class Bucket:
    capacity: int
    rate: int
    tokens: int
    last: int


@dataclass(frozen=True)
class Keyed:
    capacity: int
    rate: int
    buckets: List[Tuple[str, Bucket]]


class LimitErr(Exception):
    pass


class ClockSkew(LimitErr):
    def __init__(self, last: int, now: int):
        super().__init__(last, now)
        self.last = last
        self.now = now


class TooLarge(LimitErr):
    def __init__(self, n: int, capacity: int):
        super().__init__(n, capacity)
        self.n = n
        self.capacity = capacity


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
    if n > b.capacity:
        raise TooLarge(n, b.capacity)
    r = _refill(b, now)
    if r.tokens >= n:
        return 0
    return (n - r.tokens + r.rate - 1) // r.rate


def new_keyed(capacity: int, rate: int) -> Keyed:
    return Keyed(capacity, rate, [])


def take_key(k: Keyed, key: str, now: int) -> Tuple[bool, Keyed]:
    b = next((b for name, b in k.buckets if name == key), None) or new_bucket(k.capacity, k.rate, now)
    ok, b2 = take(b, now, 1)
    return ok, replace(k, buckets=[(n, x) for n, x in k.buckets if n != key] + [(key, b2)])


if __name__ == "__main__":
    print(" ".join(str(b).lower() for b in run_requests(new_window(3, 10), [0, 1, 2, 3, 12])))


def test_first_allowed():
    assert allow(new_window(3, 10), 0)[0]


def test_counts():
    assert allow(new_window(3, 10), 0)[1].count == 1


def test_new_window_resets():
    assert run_requests(new_window(1, 10), [0, 10])[-1] is True


def test_limit_holds():
    assert run_requests(new_window(1, 10), [0, 1]) == [True, False]


def test_bucket_take():
    assert take(new_bucket(2, 1, 0), 0, 3)[0] is False


def test_retry():
    assert retry_after(take(new_bucket(4, 2, 0), 0, 4)[1], 0, 3) == 2


def test_keyed():
    assert take_key(new_keyed(1, 1), "a", 0)[0]
