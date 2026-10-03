from dataclasses import replace
from app import *


def with_coupon(c):
    return replace(sample_order(), coupon=c)


def big(c, unit):
    return Order(9, [Line("tv", 1, unit)], c)


def test_hidden_gross():
    assert gross(sample_order()) == 3150


def test_hidden_plain_total():
    assert total(sample_order()) == 3901


def test_hidden_plain_invoice():
    assert invoice(sample_order()) == "order 7: gross 31.50, discount 0.00, shipping 4.99, tax 2.52, total 39.01"


def test_hidden_save10():
    o = with_coupon("SAVE10")
    assert (discount(o), tax(o), total(o)) == (315, 226, 3560)


def test_hidden_save10_invoice():
    assert invoice(with_coupon("SAVE10")) == "order 7: gross 31.50, discount 3.15, shipping 4.99, tax 2.26, total 35.60"


def test_hidden_big20_small():
    assert discount(with_coupon("BIG20")) == 0


def test_hidden_big20():
    o = big("BIG20", 12000)
    assert (discount(o), shipping(o), total(o)) == (2400, 0, 10368)


def test_hidden_net_threshold():
    o = big("SAVE10", 5400)
    assert (shipping(o), total(o)) == (499, 5747)


def test_hidden_gross_threshold():
    assert shipping(big("", 5000)) == 0


def test_hidden_freeship():
    o = with_coupon("FREESHIP")
    assert (shipping(o), discount(o), total(o)) == (0, 0, 3402)


def test_hidden_unknown():
    assert total(with_coupon("XYZ")) == 3901
