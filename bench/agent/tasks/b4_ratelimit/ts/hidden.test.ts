import { test } from "node:test";
import assert from "node:assert/strict";
import { ClockSkew, LimitErr, TooLarge, newBucket, newKeyed, newWindow, retryAfter, runRequests, take, takeKey } from "./app";

function hDrained(capacity: number, rate: number) {
  return take(newBucket(capacity, rate, 0), 0, capacity)[1];
}

function hKeys(steps: [string, number][]): boolean[] {
  let k = newKeyed(2, 1);
  const out: boolean[] = [];
  for (const [key, now] of steps) {
    const [ok, k2] = takeKey(k, key, now);
    k = k2;
    out.push(ok);
  }
  return out;
}

test("hidden_window_limit", () => {
  assert.deepEqual(runRequests(newWindow(2, 10), [0, 1, 2]), [true, true, false]);
});

test("hidden_window_reset", () => {
  assert.deepEqual(runRequests(newWindow(2, 10), [0, 1, 2, 10, 11, 12]), [true, true, false, true, true, false]);
});

test("hidden_window_skew", () => {
  assert.throws(
    () => runRequests(newWindow(2, 10), [10, 3]),
    (e) => e instanceof ClockSkew && e.last === 10 && e.now === 3 && e instanceof LimitErr,
  );
});

test("hidden_bucket_full", () => {
  const b = newBucket(5, 1, 100);
  assert.equal(b.tokens, 5);
  assert.equal(b.last, 100);
  assert.equal(b.capacity, 5);
  assert.equal(b.rate, 1);
});

test("hidden_take_ok", () => {
  const [ok, b] = take(newBucket(5, 1, 0), 0, 3);
  assert.equal(ok, true);
  assert.equal(b.tokens, 2);
});

test("hidden_take_short", () => {
  const [ok, b] = take(take(newBucket(5, 1, 0), 0, 3)[1], 0, 3);
  assert.equal(ok, false);
  assert.equal(b.tokens, 2);
});

test("hidden_refill", () => {
  const [ok, b] = take(hDrained(5, 2), 2, 3);
  assert.equal(ok, true);
  assert.equal(b.tokens, 1);
  assert.equal(b.last, 2);
});

test("hidden_refill_cap", () => {
  assert.equal(take(newBucket(5, 2, 0), 100, 1)[1].tokens, 4);
});

test("hidden_failed_take_refills", () => {
  const [ok, b] = take(hDrained(5, 1), 3, 4);
  assert.equal(ok, false);
  assert.equal(b.tokens, 3);
  assert.equal(b.last, 3);
});

test("hidden_take_skew", () => {
  assert.throws(() => take(newBucket(5, 1, 10), 9, 1), (e) => e instanceof ClockSkew && e.last === 10 && e.now === 9);
});

test("hidden_retry_now", () => {
  assert.equal(retryAfter(newBucket(5, 1, 0), 0, 3), 0);
});

test("hidden_retry_wait", () => {
  assert.equal(retryAfter(hDrained(5, 2), 0, 3), 2);
  assert.equal(retryAfter(hDrained(5, 2), 1, 3), 1);
  assert.equal(retryAfter(hDrained(5, 2), 0, 4), 2);
});

test("hidden_retry_too_large", () => {
  assert.throws(
    () => retryAfter(newBucket(5, 1, 0), 0, 6),
    (e) => e instanceof TooLarge && e.n === 6 && e.capacity === 5 && e instanceof LimitErr,
  );
});

test("hidden_retry_skew", () => {
  assert.throws(() => retryAfter(newBucket(5, 1, 10), 4, 1), (e) => e instanceof ClockSkew && e.last === 10 && e.now === 4);
});

test("hidden_keyed", () => {
  assert.deepEqual(hKeys([["x", 0], ["x", 0], ["x", 0], ["y", 0]]), [true, true, false, true]);
});

test("hidden_keyed_refill", () => {
  assert.deepEqual(hKeys([["x", 0], ["x", 0], ["x", 0], ["x", 3]]), [true, true, false, true]);
});

test("hidden_keyed_buckets", () => {
  assert.equal(takeKey(takeKey(newKeyed(2, 1), "x", 0)[1], "y", 5)[1].buckets.length, 2);
  assert.deepEqual([...newKeyed(3, 2).buckets], []);
  assert.equal(newKeyed(3, 2).capacity, 3);
});
