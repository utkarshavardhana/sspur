import { test } from "node:test";
import assert from "node:assert/strict";
import { summary, topK, wordFreq } from "./app";

test("freq basic", () => {
  assert.deepEqual(wordFreq("a b a"), [["a", 2], ["b", 1]]);
});

test("top one", () => {
  assert.deepEqual(topK("x y y z", 1), [["y", 2]]);
});

test("summary basic", () => {
  assert.equal(summary("b a b"), "3 words, 2 distinct, top: b=2 a=1");
});
