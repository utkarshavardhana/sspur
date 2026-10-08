import { test } from "node:test";
import assert from "node:assert/strict";
import { UnknownSku, describe, findItem, item, restock, sample } from "./app";

test("restock adds", () => {
  assert.equal(findItem(restock(sample(), "a1", 3), "a1")?.qty, 5);
});

test("restock unknown", () => {
  assert.throws(() => restock(sample(), "zz", 1), (e) => e instanceof UnknownSku && e.sku === "zz");
});

test("describe one", () => {
  assert.equal(describe(item("x", "nut", 1, 2)), "x nut x1 @ 2");
});
