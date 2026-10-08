import { test } from "node:test";
import assert from "node:assert/strict";
import { newGame, play, run, score, world } from "./app";

const HALL = "You are in the hall. Exits: east, north.";
const KITCHEN = "You are in the kitchen. Exits: west. Items: bread, key.";
const NOPE = "I don't understand.";

test("hidden_look_sorted", () => {
  assert.deepEqual(play(["look"]), [HALL]);
});

test("hidden_items_shown", () => {
  assert.deepEqual(play(["go east"]), [KITCHEN]);
});

test("hidden_case_spaces", () => {
  assert.deepEqual(play(["  GO   East ", "LOOK", "Look  "]), [KITCHEN, KITCHEN, KITCHEN]);
});

test("hidden_unknown", () => {
  assert.deepEqual(play(["dance", "", "go", "go east west"]), [NOPE, NOPE, NOPE, NOPE]);
});

test("hidden_no_exit", () => {
  assert.deepEqual(play(["go west"]), ["You can't go that way."]);
});

test("hidden_take", () => {
  assert.deepEqual(play(["go east", "take key", "look"]), [KITCHEN, "Taken.", "You are in the kitchen. Exits: west. Items: bread."]);
});

test("hidden_take_case", () => {
  assert.deepEqual(play(["go east", "TAKE Key", "inventory"]).slice(1), ["Taken.", "You carry: key."]);
});

test("hidden_take_missing", () => {
  assert.deepEqual(play(["take key"]), ["There is no key here."]);
});

test("hidden_inventory_empty", () => {
  assert.deepEqual(play(["inventory"]), ["You carry nothing."]);
});

test("hidden_inventory_sorted", () => {
  assert.equal(play(["go east", "take key", "take bread", "inventory"]).at(-1), "You carry: bread, key.");
});

test("hidden_drop", () => {
  assert.deepEqual(play(["go east", "take key", "go west", "drop key", "look", "inventory"]).slice(2), [
    HALL,
    "Dropped.",
    "You are in the hall. Exits: east, north. Items: key.",
    "You carry nothing.",
  ]);
});

test("hidden_drop_missing", () => {
  assert.deepEqual(play(["drop gold"]), ["You don't have gold."]);
});

test("hidden_locked", () => {
  assert.deepEqual(play(["go north"]), ["The door is locked."]);
  assert.equal(run(["go north"])[0].room, "hall");
});

test("hidden_unlocked", () => {
  assert.equal(play(["go east", "take key", "go west", "go north"]).at(-1), "You are in the vault. Exits: south. Items: gold.");
});

test("hidden_moves", () => {
  assert.equal(run(["go east", "go up", "go west", "go north"])[0].moves, 2);
});

test("hidden_world_locked", () => {
  assert.deepEqual(world().map((r) => [...r.locked]), [["north"], [], []]);
});

test("hidden_score", () => {
  assert.equal(score(newGame()), 0);
  assert.equal(score(run(["go east", "take key", "take bread"])[0]), 20);
  assert.equal(score(run(["go east", "take key", "go west", "go north", "take gold"])[0]), 45);
});
