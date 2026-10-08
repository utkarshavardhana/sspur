import { test } from "node:test";
import assert from "node:assert/strict";
import { DuplicateSku, InvErr, OutOfStock, UnknownSku, addItem, findItem, item, lowStock, removeStock, restock, sample, totalValue } from "./app";

test("hidden_total", () => {
  assert.equal(totalValue(sample()), 7 * 12 + 40 * 3 + 2 * 250);
});

test("hidden_total_empty", () => {
  assert.equal(totalValue([]), 0);
});

test("hidden_low_sorted", () => {
  assert.deepEqual(lowStock(sample(), 10), ["a1", "c3"]);
});

test("hidden_low_none", () => {
  assert.deepEqual(lowStock(sample(), 1), []);
});

test("hidden_remove_ok", () => {
  assert.equal(findItem(removeStock(sample(), "b2", 15), "b2")?.qty, 25);
});

test("hidden_remove_all", () => {
  assert.equal(findItem(removeStock(sample(), "a1", 2), "a1")?.qty, 0);
});

test("hidden_remove_keeps_order", () => {
  assert.deepEqual(removeStock(sample(), "b2", 1).map((i) => i.sku), ["c3", "b2", "a1"]);
});

test("hidden_remove_short", () => {
  assert.throws(
    () => removeStock(sample(), "a1", 3),
    (e) => e instanceof OutOfStock && e.sku === "a1" && e.wanted === 3 && e.have === 2,
  );
});

test("hidden_remove_unknown", () => {
  assert.throws(() => removeStock(sample(), "q9", 1), (e) => e instanceof UnknownSku && e.sku === "q9");
});

test("hidden_add_ok", () => {
  assert.deepEqual(addItem(sample(), item("d4", "drill", 1, 900)).map((i) => i.sku), ["c3", "b2", "a1", "d4"]);
});

test("hidden_add_dup", () => {
  assert.throws(
    () => addItem(sample(), item("b2", "bolt", 1, 3)),
    (e) => e instanceof DuplicateSku && e.sku === "b2" && e instanceof InvErr,
  );
});

test("hidden_restock_still", () => {
  assert.equal(findItem(restock(sample(), "c3", 3), "c3")?.qty, 10);
});
