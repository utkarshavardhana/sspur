from dataclasses import dataclass
from decimal import Decimal
from typing import Protocol


@dataclass(frozen=True)
class Item:
    sku: str
    qty: int
    price: Decimal


@dataclass(frozen=True)
class Order:
    id: str
    items: list[Item]


class OrderError(Exception):
    pass


class EmptyOrder(OrderError):
    pass


class OutOfStock(OrderError):
    def __init__(self, sku: str):
        super().__init__(sku)
        self.sku = sku


class Store(Protocol):
    def stock(self, sku: str) -> int: ...
    def put_order(self, order: Order) -> None: ...


def total(items: list[Item]) -> Decimal:
    return sum((i.price * i.qty for i in items), Decimal(0))


def place(store: Store, order: Order) -> Order:
    if not order.items:
        raise EmptyOrder()
    for i in order.items:
        if i.qty <= 0:
            raise ValueError(f"bad qty for {i.sku}")
        if store.stock(i.sku) < i.qty:
            raise OutOfStock(i.sku)
    store.put_order(order)
    return order
