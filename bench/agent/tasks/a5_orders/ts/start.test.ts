import { test } from "node:test";
import assert from "node:assert/strict";
import { invoice, money, sampleOrder, subtotal, total } from "./app";

test("subtotal sample", () => {
  assert.equal(subtotal(sampleOrder()), 3150);
});

test("total sample", () => {
  assert.equal(total(sampleOrder()), 3150 + 499 + 252);
});

test("invoice sample", () => {
  assert.equal(invoice(sampleOrder()), "order 7: subtotal 31.50, shipping 4.99, tax 2.52, total 39.01");
});

test("money pad", () => {
  assert.equal(money(1205), "12.05");
});
