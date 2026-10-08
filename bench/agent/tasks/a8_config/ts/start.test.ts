import { test } from "node:test";
import assert from "node:assert/strict";
import { Missing, getStr, lookup, parse, sampleText } from "./app";

test("parse sample", () => {
  assert.equal(parse(sampleText()).length, 3);
});

test("lookup host", () => {
  assert.equal(lookup(parse(sampleText()), "host"), "localhost");
});

test("missing", () => {
  assert.throws(() => getStr(parse(sampleText()), "nope"), (e) => e instanceof Missing && e.key === "nope");
});
