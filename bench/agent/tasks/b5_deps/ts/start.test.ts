import { test } from "node:test";
import assert from "node:assert/strict";
import { installOrder, plan, registry } from "./app";

test("leaf", () => {
  assert.deepEqual(installOrder(registry(), "http"), ["http"]);
});

test("db first log", () => {
  assert.deepEqual(installOrder(registry(), "db"), ["log", "db"]);
});

test("unknown", () => {
  assert.equal(plan(registry(), "nope"), "error: unknown package nope");
});
