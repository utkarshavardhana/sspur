from dataclasses import dataclass, replace
from typing import List, Optional


@dataclass(frozen=True)
class Item:
    sku: str
    name: str
    qty: int
    price: int


class InvErr(Exception):
    pass


class UnknownSku(InvErr):
    def __init__(self, sku: str):
        super().__init__(sku)
        self.sku = sku


class OutOfStock(InvErr):
    def __init__(self, sku: str, wanted: int, have: int):
        super().__init__(sku, wanted, have)
        self.sku = sku
        self.wanted = wanted
        self.have = have


class DuplicateSku(InvErr):
    def __init__(self, sku: str):
        super().__init__(sku)
        self.sku = sku


def sample() -> List[Item]:
    return [Item("c3", "cable", 7, 12), Item("b2", "bolt", 40, 3), Item("a1", "anchor", 2, 250)]


def find_item(inv: List[Item], sku: str) -> Optional[Item]:
    return next((i for i in inv if i.sku == sku), None)


def total_value(inv: List[Item]) -> int:
    return sum(i.qty * i.price for i in inv)


def restock(inv: List[Item], sku: str, n: int) -> List[Item]:
    assert n > 0
    if find_item(inv, sku) is None:
        raise UnknownSku(sku)
    return [replace(i, qty=i.qty + n) if i.sku == sku else i for i in inv]


def low_stock(inv: List[Item], threshold: int) -> List[str]:
    return sorted(i.sku for i in inv if i.qty < threshold)


def remove_stock(inv: List[Item], sku: str, n: int) -> List[Item]:
    assert n > 0
    item = find_item(inv, sku)
    if item is None:
        raise UnknownSku(sku)
    if n > item.qty:
        raise OutOfStock(sku, wanted=n, have=item.qty)
    return [replace(i, qty=i.qty - n) if i.sku == sku else i for i in inv]


def add_item(inv: List[Item], item: Item) -> List[Item]:
    if find_item(inv, item.sku) is not None:
        raise DuplicateSku(item.sku)
    return list(inv) + [item]


def describe(i: Item) -> str:
    return f"{i.sku} {i.name} x{i.qty} @ {i.price}"


def report(inv: List[Item]) -> str:
    return "\n".join(describe(i) for i in inv)


if __name__ == "__main__":
    print(report(sample()))


def test_restock_adds():
    assert find_item(restock(sample(), "a1", 3), "a1").qty == 5


def test_restock_unknown():
    try:
        restock(sample(), "zz", 1)
        assert False
    except UnknownSku as e:
        assert e.sku == "zz"


def test_describe_one():
    assert describe(Item("x", "nut", 1, 2)) == "x nut x1 @ 2"


def test_total_value():
    assert total_value(sample()) == 7 * 12 + 40 * 3 + 2 * 250


def test_low_stock_sorted():
    assert low_stock(sample(), 10) == ["a1", "c3"]


def test_remove_stock_ok():
    r = remove_stock(sample(), "b2", 15)
    assert [i.sku for i in r] == ["c3", "b2", "a1"]
    assert find_item(r, "b2").qty == 25


def test_remove_stock_unknown():
    try:
        remove_stock(sample(), "zz", 1)
        assert False
    except UnknownSku as e:
        assert e.sku == "zz"


def test_remove_stock_out():
    try:
        remove_stock(sample(), "a1", 3)
        assert False
    except OutOfStock as e:
        assert (e.sku, e.wanted, e.have) == ("a1", 3, 2)


def test_add_item_appends():
    r = add_item(sample(), Item("d4", "drill", 1, 90))
    assert [i.sku for i in r] == ["c3", "b2", "a1", "d4"]


def test_add_item_duplicate():
    try:
        add_item(sample(), Item("b2", "x", 1, 1))
        assert False
    except DuplicateSku as e:
        assert e.sku == "b2"
