import { test } from "node:test";
import assert from "node:assert/strict";
import { Priority, complete, demo, pending, render, task } from "./app";

test("ids", () => {
  assert.deepEqual(demo().map((t) => t.id), [1, 2, 3, 4]);
});

test("complete one", () => {
  assert.deepEqual(pending(complete(demo(), 2)).map((t) => t.id), [1, 3, 4]);
});

test("render one", () => {
  assert.equal(render(task(5, "nap", true, Priority.Low)), "[x] #5 nap");
});
