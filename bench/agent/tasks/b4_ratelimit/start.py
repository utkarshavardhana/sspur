from dataclasses import dataclass, replace
from typing import List, Tuple


@dataclass(frozen=True)
class Window:
    limit: int
    size: int
    start: int
    count: int


def new_window(limit: int, size: int) -> Window:
    return Window(limit, size, 0, 0)


def allow(w: Window, now: int) -> Tuple[bool, Window]:
    w2 = replace(w, start=now, count=0) if now >= w.start + w.size else w
    if w2.count <= w2.limit:
        return True, replace(w2, count=w2.count + 1)
    return False, w2


def run_requests(w: Window, times: List[int]) -> List[bool]:
    cur = w
    out = []
    for t in times:
        ok, cur = allow(cur, t)
        out.append(ok)
    return out


if __name__ == "__main__":
    print(" ".join(str(b).lower() for b in run_requests(new_window(3, 10), [0, 1, 2, 3, 12])))


def test_first_allowed():
    assert allow(new_window(3, 10), 0)[0]


def test_counts():
    assert allow(new_window(3, 10), 0)[1].count == 1


def test_new_window_resets():
    assert run_requests(new_window(1, 10), [0, 10])[-1] is True
