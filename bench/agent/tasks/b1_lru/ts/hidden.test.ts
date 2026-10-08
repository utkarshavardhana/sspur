import { test } from "node:test";
import assert from "node:assert/strict";
import { BadCapacity, CacheErr, describe, getOr, keys, lookup, newCache, peek, put, resize, size } from "./app";

function hAbc(cap: number) {
  return put(put(put(newCache(cap), "a", 1), "b", 2), "c", 3);
}

function hCounted() {
  return lookup(lookup(lookup(hAbc(3), "a")[1], "z")[1], "b")[1];
}

test("hidden_bad_cap_zero", () => {
  assert.throws(() => newCache(0), (e) => e instanceof BadCapacity && e.cap === 0 && e instanceof CacheErr);
});

test("hidden_bad_cap_neg", () => {
  assert.throws(() => newCache(-3), (e) => e instanceof BadCapacity && e.cap === -3);
});

test("hidden_cap_one", () => {
  const c = put(put(newCache(1), "a", 1), "b", 2);
  assert.equal(size(c), 1);
  assert.deepEqual(keys(c), ["b"]);
});

test("hidden_evict", () => {
  assert.deepEqual(keys(hAbc(2)), ["b", "c"]);
});

test("hidden_no_evict", () => {
  assert.deepEqual(keys(hAbc(3)), ["a", "b", "c"]);
});

test("hidden_update_existing", () => {
  assert.deepEqual(keys(put(hAbc(3), "a", 9)), ["b", "c", "a"]);
  assert.equal(peek(put(hAbc(3), "a", 9), "a"), 9);
});

test("hidden_update_full", () => {
  assert.equal(size(put(hAbc(3), "b", 5)), 3);
  assert.deepEqual(keys(put(hAbc(3), "b", 5)), ["a", "c", "b"]);
});

test("hidden_lookup_recency", () => {
  assert.deepEqual(keys(put(lookup(hAbc(3), "a")[1], "d", 4)), ["c", "a", "d"]);
});

test("hidden_lookup_value", () => {
  assert.equal(lookup(hAbc(3), "b")[0], 2);
});

test("hidden_lookup_miss", () => {
  assert.equal(lookup(hAbc(3), "z")[0], undefined);
  assert.deepEqual(keys(lookup(hAbc(3), "z")[1]), ["a", "b", "c"]);
});

test("hidden_counters", () => {
  assert.equal(hCounted().hits, 2);
  assert.equal(hCounted().misses, 1);
});

test("hidden_counters_start", () => {
  assert.equal(newCache(2).hits, 0);
  assert.equal(newCache(2).misses, 0);
});

test("hidden_get_or_miss", () => {
  const [v, c] = getOr(hAbc(3), "z", -1);
  assert.equal(v, -1);
  assert.equal(c.misses, 1);
  assert.equal(c.hits, 0);
});

test("hidden_get_or_hit", () => {
  const [v, c] = getOr(hAbc(3), "a", -1);
  assert.equal(v, 1);
  assert.equal(c.hits, 1);
  assert.deepEqual(keys(c), ["b", "c", "a"]);
});

test("hidden_peek", () => {
  assert.equal(peek(hAbc(3), "a"), 1);
  assert.equal(peek(hAbc(3), "z"), undefined);
  assert.equal(peek(hCounted(), "a"), 1);
});

test("hidden_resize_down", () => {
  assert.deepEqual(keys(resize(hAbc(3), 1)), ["c"]);
  assert.equal(resize(hAbc(3), 1).cap, 1);
});

test("hidden_resize_then_put", () => {
  assert.deepEqual(keys(put(resize(hAbc(3), 2), "d", 4)), ["c", "d"]);
});

test("hidden_resize_up", () => {
  assert.deepEqual(keys(put(resize(hAbc(2), 3), "d", 4)), ["b", "c", "d"]);
});

test("hidden_resize_bad", () => {
  assert.throws(() => resize(hAbc(2), 0), (e) => e instanceof BadCapacity && e.cap === 0);
});

test("hidden_put_keeps_counters", () => {
  assert.equal(put(hCounted(), "q", 1).hits, 2);
  assert.equal(resize(hCounted(), 1).misses, 1);
});

test("hidden_describe", () => {
  assert.equal(describe(hAbc(2)), "cap=2 [b=2, c=3]");
});
