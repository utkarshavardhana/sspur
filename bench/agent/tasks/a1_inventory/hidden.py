import pytest
from app import *


def test_hidden_total():
    assert total_value(sample()) == 7 * 12 + 40 * 3 + 2 * 250


def test_hidden_total_empty():
    assert total_value([]) == 0


def test_hidden_low_sorted():
    assert low_stock(sample(), 10) == ["a1", "c3"]


def test_hidden_low_none():
    assert low_stock(sample(), 1) == []


def test_hidden_remove_ok():
    assert find_item(remove_stock(sample(), "b2", 15), "b2").qty == 25


def test_hidden_remove_all():
    assert find_item(remove_stock(sample(), "a1", 2), "a1").qty == 0


def test_hidden_remove_keeps_order():
    assert [i.sku for i in remove_stock(sample(), "b2", 1)] == ["c3", "b2", "a1"]


def test_hidden_remove_short():
    with pytest.raises(OutOfStock) as e:
        remove_stock(sample(), "a1", 3)
    assert (e.value.sku, e.value.wanted, e.value.have) == ("a1", 3, 2)


def test_hidden_remove_unknown():
    with pytest.raises(UnknownSku) as e:
        remove_stock(sample(), "q9", 1)
    assert e.value.sku == "q9"


def test_hidden_add_ok():
    assert [i.sku for i in add_item(sample(), Item("d4", "drill", 1, 900))] == ["c3", "b2", "a1", "d4"]


def test_hidden_add_dup():
    with pytest.raises(DuplicateSku) as e:
        add_item(sample(), Item("b2", "bolt", 1, 3))
    assert e.value.sku == "b2"
    assert isinstance(e.value, InvErr)


def test_hidden_restock_still():
    assert find_item(restock(sample(), "c3", 3), "c3").qty == 10
