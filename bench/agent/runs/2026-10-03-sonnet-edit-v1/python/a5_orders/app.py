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
    if o.coupon == "BIG20" and g >= 10000:
        return g * 20 // 100
    return 0


def net(o: Order) -> int:
    return gross(o) - discount(o)


def shipping(o: Order) -> int:
    return 0 if net(o) >= 5000 or o.coupon == "FREESHIP" else 499


def tax(o: Order) -> int:
    return net(o) * 8 // 100


def total(o: Order) -> int:
    return net(o) + shipping(o) + tax(o)


def money(cents: int) -> str:
    assert cents >= 0
    return f"{cents // 100}.{cents % 100:02d}"


def invoice(o: Order) -> str:
    return (f"order {o.id}: gross {money(gross(o))}, discount {money(discount(o))}, "
            f"shipping {money(shipping(o))}, tax {money(tax(o))}, total {money(total(o))}")


def sample_order() -> Order:
    return Order(7, [Line("pen", 3, 250), Line("pad", 2, 1200)], "")


if __name__ == "__main__":
    print(invoice(sample_order()))


def _o(coupon, unit=1000, qty=1):
    return Order(1, [Line("x", qty, unit)], coupon)


def test_gross_sample():
    assert gross(sample_order()) == 3150


def test_total_sample():
    assert total(sample_order()) == 3150 + 499 + 252


def test_invoice_sample():
    assert invoice(sample_order()) == "order 7: gross 31.50, discount 0.00, shipping 4.99, tax 2.52, total 39.01"


def test_money_pad():
    assert money(1205) == "12.05"


def test_save10():
    assert discount(_o("SAVE10", 1999)) == 199


def test_big20():
    assert discount(_o("BIG20", 9999)) == 0
    assert discount(_o("BIG20", 10000)) == 2000


def test_other_coupons():
    assert discount(_o("")) == 0
    assert discount(_o("FREESHIP")) == 0


def test_shipping_rules():
    assert shipping(_o("FREESHIP")) == 0
    assert shipping(_o("", 5000)) == 0
    assert shipping(_o("SAVE10", 5000)) == 499
    assert shipping(_o("SAVE10", 5556)) == 0


def test_total_and_invoice_coupon():
    o = _o("BIG20", 10000)
    assert total(o) == 8000 + 0 + 640
    assert invoice(o) == "order 1: gross 100.00, discount 20.00, shipping 0.00, tax 6.40, total 86.40"
