import { test } from "node:test";
import assert from "node:assert/strict";
import { money, shipFee, taxRate } from "../shop/common";
import { customerNew, customerSample, customerTax, customerTotal } from "../shop/customer";
import { invoiceGross, invoiceNew } from "../shop/invoice";
import { lessonBucket, lessonNew } from "../shop/lesson";
import { orderExpressShipping, orderNew, orderShipping } from "../shop/order";
import { parcelNew, parcelShipping } from "../shop/parcel";
import { productLine, productNew } from "../shop/product";
import { rmaNew, rmaShipping } from "../shop/rma";
import { routeSample, routeTotal } from "../shop/route";
import { warehouseDeactivate, warehouseNew, warehouseRestockAll } from "../shop/warehouse";

test("hidden_eu_rate", () => {
  assert.ok(taxRate("EU") === 21 && taxRate("US") === 7 && taxRate("UK") === 20 && taxRate("APAC") === 0);
});

test("hidden_eu_used", () => {
  assert.ok(customerTax(customerNew(1, "a", 10, 100, "EU")) === 210 && invoiceGross(invoiceNew(1, "a", 1, 1000, "EU")) === 1210);
});

test("hidden_money", () => {
  assert.ok(money(1005) === "10.05" && money(7) === "0.07" && money(123400) === "1234.00" && money(1999) === "19.99" && money(0) === "0.00");
});

test("hidden_money_used", () => {
  assert.equal(productLine(productNew(4, "pen", 1, 105, "APAC")), "pen #4: 1.05");
});

test("hidden_ship_fee", () => {
  assert.ok(shipFee(10, false) === 449 && shipFee(10, true) === 898 && shipFee(0, true) === 0);
});

test("hidden_shipping_callers", () => {
  assert.ok(
    orderShipping(orderNew(1, "o", 10, 1, "US")) === 449 &&
      parcelShipping(parcelNew(2, "p", 4, 1, "US")) === 359 &&
      rmaShipping(rmaNew(3, "r", 1, 1, "UK")) === 314,
  );
});

test("hidden_express", () => {
  assert.ok(orderExpressShipping(orderNew(1, "o", 10, 1, "US")) === 898 && orderExpressShipping(orderNew(2, "z", 0, 1, "US")) === 0);
});

test("hidden_restock_all", () => {
  const ws = [warehouseNew(1, "a", 5, 1, "US"), warehouseDeactivate(warehouseNew(2, "b", 5, 1, "US")), warehouseNew(3, "c", 0, 1, "EU")];
  assert.deepEqual(warehouseRestockAll(ws, 4).map((w) => w.qty), [9, 5, 4]);
});

test("hidden_restock_all_keeps", () => {
  assert.deepEqual(warehouseRestockAll([warehouseNew(7, "x", 1, 2, "US"), warehouseNew(8, "y", 2, 3, "UK")], 1).map((w) => w.id), [7, 8]);
  assert.deepEqual([...warehouseRestockAll([], 3)], []);
});

test("hidden_unchanged", () => {
  assert.ok(customerTotal(customerSample()) === 886 && routeTotal(routeSample()) === 1308 && lessonBucket(lessonNew(1, "l", 1, 1, "US")) === "small");
});
