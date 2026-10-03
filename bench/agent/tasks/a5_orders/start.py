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


def subtotal(o: Order) -> int:
    return sum(l.qty * l.unit for l in o.lines)


def shipping(o: Order) -> int:
    return 0 if subtotal(o) >= 5000 else 499


def tax(o: Order) -> int:
    return subtotal(o) * 8 // 100


def total(o: Order) -> int:
    return subtotal(o) + shipping(o) + tax(o)


def money(cents: int) -> str:
    assert cents >= 0
    return f"{cents // 100}.{cents % 100:02d}"


def invoice(o: Order) -> str:
    return f"order {o.id}: subtotal {money(subtotal(o))}, shipping {money(shipping(o))}, tax {money(tax(o))}, total {money(total(o))}"


def sample_order() -> Order:
    return Order(7, [Line("pen", 3, 250), Line("pad", 2, 1200)], "")


if __name__ == "__main__":
    print(invoice(sample_order()))


def test_subtotal_sample():
    assert subtotal(sample_order()) == 3150


def test_total_sample():
    assert total(sample_order()) == 3150 + 499 + 252


def test_invoice_sample():
    assert invoice(sample_order()) == "order 7: subtotal 31.50, shipping 4.99, tax 2.52, total 39.01"


def test_money_pad():
    assert money(1205) == "12.05"
