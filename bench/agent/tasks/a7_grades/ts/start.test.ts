import { test } from "node:test";
import assert from "node:assert/strict";
import { letter, ranking, reportCard, roster, student } from "./app";

test("card", () => {
  assert.equal(reportCard(student("x", [50, 60])), "x: 55 (F)");
});

test("rank order", () => {
  assert.deepEqual(ranking(roster()).slice(0, 2), ["bo", "mia"]);
});

test("letter b", () => {
  assert.equal(letter(85), "B");
});
