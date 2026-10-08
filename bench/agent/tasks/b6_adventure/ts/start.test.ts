import { test } from "node:test";
import assert from "node:assert/strict";
import { play, run } from "./app";

test("starts in hall", () => {
  assert.deepEqual(play(["look"]), ["You are in the hall. Exits: north, east."]);
});

test("walks", () => {
  assert.equal(play(["go east", "go west"]).at(-1), "You are in the hall. Exits: north, east.");
});

test("blocked", () => {
  assert.deepEqual(play(["go up"]), ["You can't go that way."]);
});

test("counts moves", () => {
  assert.equal(run(["go east", "go west"])[0].moves, 2);
});
