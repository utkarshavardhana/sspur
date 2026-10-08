import { test } from "node:test";
import assert from "node:assert/strict";
import { describe, getOr, keys, lookup, newCache, put } from "./app";

test("put two", () => {
  assert.deepEqual(keys(put(put(newCache(2), "a", 1), "b", 2)), ["a", "b"]);
});

test("lookup hit", () => {
  assert.equal(lookup(put(newCache(2), "a", 1), "a"), 1);
});

test("lookup miss", () => {
  assert.equal(getOr(newCache(2), "z", -1), -1);
});

test("described", () => {
  assert.equal(describe(put(newCache(3), "x", 7)), "cap=3 [x=7]");
});
