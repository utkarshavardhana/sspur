import { test } from "node:test";
import assert from "node:assert/strict";
import { average, curve, dropLowest, grade, honorRoll, letter, ranking, reportCard, roster, student } from "./app";

test("hidden_empty", () => {
  assert.equal(average(student("e", [])), 0);
  assert.equal(reportCard(student("e", [])), "e: 0 (F)");
});

test("hidden_bounds", () => {
  assert.deepEqual([90, 89, 80, 70, 60, 59, 100].map(letter), ["A", "B", "B", "C", "D", "F", "A"]);
});

test("hidden_rank", () => {
  assert.deepEqual(ranking(roster()), ["bo", "mia", "zed", "ali"]);
});

test("hidden_rank_ties", () => {
  assert.deepEqual(ranking([student("cy", [80]), student("al", [80]), student("bo", [90])]), ["bo", "al", "cy"]);
});

test("hidden_drop", () => {
  assert.deepEqual(dropLowest(student("z", [95, 50, 90, 50])).scores, [95, 90, 50]);
});

test("hidden_drop_one", () => {
  assert.deepEqual(dropLowest(student("z", [70])).scores, [70]);
  assert.deepEqual(dropLowest(student("z", [])).scores, []);
});

test("hidden_drop_name", () => {
  const s = dropLowest(student("q", [1, 2]));
  assert.deepEqual([s.name, [...s.scores]], ["q", [2]]);
});

test("hidden_curve", () => {
  const got = curve([student("a", [95, 80]), student("b", [])], 10).map((s) => [s.name, [...s.scores]]);
  assert.deepEqual(got, [["a", [100, 90]], ["b", []]]);
});

test("hidden_honor", () => {
  assert.deepEqual(honorRoll(roster()), ["bo", "mia", "zed"]);
});

test("hidden_honor_empty", () => {
  assert.deepEqual(honorRoll([]), []);
});

test("hidden_grade", () => {
  assert.equal(grade(student("m", [90, 90])), "A");
});
