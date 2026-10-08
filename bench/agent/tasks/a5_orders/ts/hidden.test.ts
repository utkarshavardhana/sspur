import { test } from "node:test";
import assert from "node:assert/strict";
import { Order, discount, gross, invoice, line, order, sampleOrder, shipping, tax, total } from "./app";

function withCoupon(c: string): Order {
  return { ...sampleOrder(), coupon: c };
}

function big(c: string, unit: number): Order {
  return order(9, [line("tv", 1, unit)], c);
}

test("hidden_gross", () => {
  assert.equal(gross(sampleOrder()), 3150);
});

test("hidden_plain_total", () => {
  assert.equal(total(sampleOrder()), 3901);
});

test("hidden_plain_invoice", () => {
  assert.equal(invoice(sampleOrder()), "order 7: gross 31.50, discount 0.00, shipping 4.99, tax 2.52, total 39.01");
});

test("hidden_save10", () => {
  const o = withCoupon("SAVE10");
  assert.deepEqual([discount(o), tax(o), total(o)], [315, 226, 3560]);
});

test("hidden_save10_invoice", () => {
  assert.equal(invoice(withCoupon("SAVE10")), "order 7: gross 31.50, discount 3.15, shipping 4.99, tax 2.26, total 35.60");
});

test("hidden_big20_small", () => {
  assert.equal(discount(withCoupon("BIG20")), 0);
});

test("hidden_big20", () => {
  const o = big("BIG20", 12000);
  assert.deepEqual([discount(o), shipping(o), total(o)], [2400, 0, 10368]);
});

test("hidden_net_threshold", () => {
  const o = big("SAVE10", 5400);
  assert.deepEqual([shipping(o), total(o)], [499, 5747]);
});

test("hidden_gross_threshold", () => {
  assert.equal(shipping(big("", 5000)), 0);
});

test("hidden_freeship", () => {
  const o = withCoupon("FREESHIP");
  assert.deepEqual([shipping(o), discount(o), total(o)], [0, 0, 3402]);
});

test("hidden_unknown", () => {
  assert.equal(total(withCoupon("XYZ")), 3901);
});
