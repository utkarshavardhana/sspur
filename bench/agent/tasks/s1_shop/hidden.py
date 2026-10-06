from shop.common import money, ship_fee, tax_rate
from shop.customer import customer_new, customer_sample, customer_tax, customer_total
from shop.invoice import invoice_gross, invoice_new
from shop.lesson import lesson_bucket, lesson_new
from shop.order import order_express_shipping, order_new, order_shipping
from shop.parcel import parcel_new, parcel_shipping
from shop.product import product_line, product_new
from shop.rma import rma_new, rma_shipping
from shop.route import route_sample, route_total
from shop.warehouse import warehouse_deactivate, warehouse_new, warehouse_restock_all


def test_hidden_eu_rate():
    assert tax_rate("EU") == 21 and tax_rate("US") == 7 and tax_rate("UK") == 20 and tax_rate("APAC") == 0


def test_hidden_eu_used():
    assert customer_tax(customer_new(1, "a", 10, 100, "EU")) == 210 and invoice_gross(invoice_new(1, "a", 1, 1000, "EU")) == 1210


def test_hidden_money():
    assert money(1005) == "10.05" and money(7) == "0.07" and money(123400) == "1234.00" and money(1999) == "19.99" and money(0) == "0.00"


def test_hidden_money_used():
    assert product_line(product_new(4, "pen", 1, 105, "APAC")) == "pen #4: 1.05"


def test_hidden_ship_fee():
    assert ship_fee(10, False) == 449 and ship_fee(10, True) == 898 and ship_fee(0, True) == 0


def test_hidden_shipping_callers():
    assert order_shipping(order_new(1, "o", 10, 1, "US")) == 449 and parcel_shipping(parcel_new(2, "p", 4, 1, "US")) == 359 and rma_shipping(rma_new(3, "r", 1, 1, "UK")) == 314


def test_hidden_express():
    assert order_express_shipping(order_new(1, "o", 10, 1, "US")) == 898 and order_express_shipping(order_new(2, "z", 0, 1, "US")) == 0


def test_hidden_restock_all():
    ws = [warehouse_new(1, "a", 5, 1, "US"), warehouse_deactivate(warehouse_new(2, "b", 5, 1, "US")), warehouse_new(3, "c", 0, 1, "EU")]
    assert [w.qty for w in warehouse_restock_all(ws, 4)] == [9, 5, 4]


def test_hidden_restock_all_keeps():
    assert [w.id for w in warehouse_restock_all([warehouse_new(7, "x", 1, 2, "US"), warehouse_new(8, "y", 2, 3, "UK")], 1)] == [7, 8]
    assert list(warehouse_restock_all([], 3)) == []


def test_hidden_unchanged():
    assert customer_total(customer_sample()) == 886 and route_total(route_sample()) == 1308 and lesson_bucket(lesson_new(1, "l", 1, 1, "US")) == "small"
