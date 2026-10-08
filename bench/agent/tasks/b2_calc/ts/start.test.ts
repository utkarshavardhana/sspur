import { test } from "node:test";
import assert from "node:assert/strict";
import { BadChar, calc, showResult } from "./app";

test("adds", () => {
  assert.equal(calc("1 + 2 + 3"), 6);
});

test("subtracts", () => {
  assert.equal(calc("10 - 4 - 3"), 3);
});

test("bad char", () => {
  assert.throws(() => calc("2 $ 3"), (e) => e instanceof BadChar && e.ch === "$");
});

test("shown", () => {
  assert.equal(showResult("7 - 2"), "7 - 2 = 5");
});
