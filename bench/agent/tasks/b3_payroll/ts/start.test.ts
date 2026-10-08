import { test } from "node:test";
import assert from "node:assert/strict";
import { parse, pay, report, row, sample } from "./app";

test("parsed", () => {
  assert.equal(parse(sample()).length, 6);
  assert.deepEqual(parse(sample())[2], row("Kathy", 400, 10));
});

test("paid", () => {
  assert.equal(pay(row("Mark", 500, 20)), 10000);
});

test("reported", () => {
  assert.equal(report(parse("Beth,400,0\nKathy,400,10")), "Beth: 0\nKathy: 4000");
});
