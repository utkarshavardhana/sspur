import pytest
from app import *


def h_abc(cap):
    return put(put(put(new_cache(cap), "a", 1), "b", 2), "c", 3)


def h_counted():
    return lookup(lookup(lookup(h_abc(3), "a")[1], "z")[1], "b")[1]


def test_hidden_bad_cap_zero():
    with pytest.raises(BadCapacity) as e:
        new_cache(0)
    assert e.value.cap == 0
    assert isinstance(e.value, CacheErr)


def test_hidden_bad_cap_neg():
    with pytest.raises(BadCapacity) as e:
        new_cache(-3)
    assert e.value.cap == -3


def test_hidden_cap_one():
    c = put(put(new_cache(1), "a", 1), "b", 2)
    assert size(c) == 1 and keys(c) == ["b"]


def test_hidden_evict():
    assert keys(h_abc(2)) == ["b", "c"]


def test_hidden_no_evict():
    assert keys(h_abc(3)) == ["a", "b", "c"]


def test_hidden_update_existing():
    assert keys(put(h_abc(3), "a", 9)) == ["b", "c", "a"]
    assert peek(put(h_abc(3), "a", 9), "a") == 9


def test_hidden_update_full():
    assert size(put(h_abc(3), "b", 5)) == 3
    assert keys(put(h_abc(3), "b", 5)) == ["a", "c", "b"]


def test_hidden_lookup_recency():
    assert keys(put(lookup(h_abc(3), "a")[1], "d", 4)) == ["c", "a", "d"]


def test_hidden_lookup_value():
    assert lookup(h_abc(3), "b")[0] == 2


def test_hidden_lookup_miss():
    assert lookup(h_abc(3), "z")[0] is None
    assert keys(lookup(h_abc(3), "z")[1]) == ["a", "b", "c"]


def test_hidden_counters():
    assert h_counted().hits == 2 and h_counted().misses == 1


def test_hidden_counters_start():
    assert new_cache(2).hits == 0 and new_cache(2).misses == 0


def test_hidden_get_or_miss():
    v, c = get_or(h_abc(3), "z", -1)
    assert v == -1 and c.misses == 1 and c.hits == 0


def test_hidden_get_or_hit():
    v, c = get_or(h_abc(3), "a", -1)
    assert v == 1 and c.hits == 1 and keys(c) == ["b", "c", "a"]


def test_hidden_peek():
    assert peek(h_abc(3), "a") == 1 and peek(h_abc(3), "z") is None and peek(h_counted(), "a") == 1


def test_hidden_resize_down():
    assert keys(resize(h_abc(3), 1)) == ["c"] and resize(h_abc(3), 1).cap == 1


def test_hidden_resize_then_put():
    assert keys(put(resize(h_abc(3), 2), "d", 4)) == ["c", "d"]


def test_hidden_resize_up():
    assert keys(put(resize(h_abc(2), 3), "d", 4)) == ["b", "c", "d"]


def test_hidden_resize_bad():
    with pytest.raises(BadCapacity) as e:
        resize(h_abc(2), 0)
    assert e.value.cap == 0


def test_hidden_put_keeps_counters():
    assert put(h_counted(), "q", 1).hits == 2 and resize(h_counted(), 1).misses == 1


def test_hidden_describe():
    assert describe(h_abc(2)) == "cap=2 [b=2, c=3]"
