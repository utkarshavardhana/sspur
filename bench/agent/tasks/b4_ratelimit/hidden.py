import pytest
from app import *


def h_drained(capacity, rate):
    return take(new_bucket(capacity, rate, 0), 0, capacity)[1]


def h_keys(steps):
    k = new_keyed(2, 1)
    out = []
    for key, now in steps:
        ok, k = take_key(k, key, now)
        out.append(ok)
    return out


def test_hidden_window_limit():
    assert run_requests(new_window(2, 10), [0, 1, 2]) == [True, True, False]


def test_hidden_window_reset():
    assert run_requests(new_window(2, 10), [0, 1, 2, 10, 11, 12]) == [True, True, False, True, True, False]


def test_hidden_window_skew():
    with pytest.raises(ClockSkew) as e:
        run_requests(new_window(2, 10), [10, 3])
    assert e.value.last == 10 and e.value.now == 3
    assert isinstance(e.value, LimitErr)


def test_hidden_bucket_full():
    b = new_bucket(5, 1, 100)
    assert b.tokens == 5 and b.last == 100 and b.capacity == 5 and b.rate == 1


def test_hidden_take_ok():
    ok, b = take(new_bucket(5, 1, 0), 0, 3)
    assert ok and b.tokens == 2


def test_hidden_take_short():
    ok, b = take(take(new_bucket(5, 1, 0), 0, 3)[1], 0, 3)
    assert not ok and b.tokens == 2


def test_hidden_refill():
    ok, b = take(h_drained(5, 2), 2, 3)
    assert ok and b.tokens == 1 and b.last == 2


def test_hidden_refill_cap():
    assert take(new_bucket(5, 2, 0), 100, 1)[1].tokens == 4


def test_hidden_failed_take_refills():
    ok, b = take(h_drained(5, 1), 3, 4)
    assert not ok and b.tokens == 3 and b.last == 3


def test_hidden_take_skew():
    with pytest.raises(ClockSkew) as e:
        take(new_bucket(5, 1, 10), 9, 1)
    assert e.value.last == 10 and e.value.now == 9


def test_hidden_retry_now():
    assert retry_after(new_bucket(5, 1, 0), 0, 3) == 0


def test_hidden_retry_wait():
    assert retry_after(h_drained(5, 2), 0, 3) == 2 and retry_after(h_drained(5, 2), 1, 3) == 1 and retry_after(h_drained(5, 2), 0, 4) == 2


def test_hidden_retry_too_large():
    with pytest.raises(TooLarge) as e:
        retry_after(new_bucket(5, 1, 0), 0, 6)
    assert e.value.n == 6 and e.value.capacity == 5
    assert isinstance(e.value, LimitErr)


def test_hidden_retry_skew():
    with pytest.raises(ClockSkew) as e:
        retry_after(new_bucket(5, 1, 10), 4, 1)
    assert e.value.last == 10 and e.value.now == 4


def test_hidden_keyed():
    assert h_keys([("x", 0), ("x", 0), ("x", 0), ("y", 0)]) == [True, True, False, True]


def test_hidden_keyed_refill():
    assert h_keys([("x", 0), ("x", 0), ("x", 0), ("x", 3)]) == [True, True, False, True]


def test_hidden_keyed_buckets():
    assert len(take_key(take_key(new_keyed(2, 1), "x", 0)[1], "y", 5)[1].buckets) == 2
    assert list(new_keyed(3, 2).buckets) == [] and new_keyed(3, 2).capacity == 3
