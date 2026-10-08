import { test } from "node:test";
import assert from "node:assert/strict";
import { allow, newWindow, runRequests } from "./app";

test("first allowed", () => {
  assert.ok(allow(newWindow(3, 10), 0)[0]);
});

test("counts", () => {
  assert.equal(allow(newWindow(3, 10), 0)[1].count, 1);
});

test("new window resets", () => {
  assert.equal(runRequests(newWindow(1, 10), [0, 10]).at(-1), true);
});
