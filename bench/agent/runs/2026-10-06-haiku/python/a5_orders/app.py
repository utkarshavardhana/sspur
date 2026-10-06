from dataclasses import dataclass
from typing import List


@dataclass(frozen=True)
class Line:
    sku: str
    qty: int
    unit: int


@dataclass(frozen=True)
class Order:
    id: int
    lines: List[Line]
    coupon: str


def gross(o: Order) -> int:
    return sum(l.qty * l.unit for l in o.lines)


def discount(o: Order) -> int:
    g = gross(o)
    if o.coupon == "SAVE10":
        return g * 10 // 100
    elif o.coupon == "BIG20":
        return g * 20 // 100 if g >= 10000 else 0
    else:
        return 0


def shipping(o: Order) -> int:
    net = gross(o) - discount(o)
    return 0 if net >= 5000 or o.coupon == "FREESHIP" else 499


def tax(o: Order) -> int:
    net = gross(o) - discount(o)
    return net * 8 // 100


def total(o: Order) -> int:
    net = gross(o) - discount(o)
    return net + shipping(o) + tax(o)


def money(cents: int) -> str:
    assert cents >= 0
    return f"{cents // 100}.{cents % 100:02d}"


def invoice(o: Order) -> str:
    g = gross(o)
    d = discount(o)
    s = shipping(o)
    t = tax(o)
    tot = total(o)
    return f"order {o.id}: gross {money(g)}, discount {money(d)}, shipping {money(s)}, tax {money(t)}, total {money(tot)}"


def sample_order() -> Order:
    return Order(7, [Line("pen", 3, 250), Line("pad", 2, 1200)], "")


if __name__ == "__main__":
    print(invoice(sample_order()))


def test_gross_sample():
    assert gross(sample_order()) == 3150


def test_total_sample():
    g = 3150
    d = 0
    s = 499
    t = g * 8 // 100
    assert total(sample_order()) == g - d + s + t


def test_invoice_sample():
    assert invoice(sample_order()) == "order 7: gross 31.50, discount 0.00, shipping 4.99, tax 2.52, total 39.01"


def test_discount_save10():
    order = Order(1, [Line("a", 1, 10000)], "SAVE10")
    assert discount(order) == 1000


def test_discount_big20_below_threshold():
    order = Order(2, [Line("b", 1, 5000)], "BIG20")
    assert discount(order) == 0


def test_discount_big20_at_threshold():
    order = Order(3, [Line("c", 1, 10000)], "BIG20")
    assert discount(order) == 2000


def test_discount_big20_above_threshold():
    order = Order(4, [Line("d", 1, 15000)], "BIG20")
    assert discount(order) == 3000


def test_discount_freeship():
    order = Order(5, [Line("e", 1, 1000)], "FREESHIP")
    assert discount(order) == 0


def test_discount_empty_coupon():
    order = Order(6, [Line("f", 1, 10000)], "")
    assert discount(order) == 0


def test_shipping_free_at_net_threshold():
    order = Order(7, [Line("g", 1, 5000)], "")
    assert shipping(order) == 0


def test_shipping_free_with_freeship():
    order = Order(8, [Line("h", 1, 1000)], "FREESHIP")
    assert shipping(order) == 0


def test_shipping_charged_below_threshold():
    order = Order(9, [Line("i", 1, 4000)], "")
    assert shipping(order) == 499


def test_total_with_save10():
    order = Order(10, [Line("j", 1, 10000)], "SAVE10")
    g = 10000
    d = 1000
    net = g - d
    s = 0
    t = net * 8 // 100
    assert total(order) == net + s + t


def test_total_with_big20():
    order = Order(11, [Line("k", 1, 10000)], "BIG20")
    g = 10000
    d = 2000
    net = g - d
    s = 0
    t = net * 8 // 100
    assert total(order) == net + s + t


def test_money_pad():
    assert money(1205) == "12.05"
