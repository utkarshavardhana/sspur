import { test } from "node:test";
import assert from "node:assert/strict";
import { AlreadyDone, NotFound, Priority, TaskErr, byPriority, clearDone, complete, demo, pending, render, renderAll, retitle, task } from "./app";

test("hidden_complete_ok", () => {
  assert.deepEqual(pending(complete(demo(), 2)).map((t) => t.id), [1, 3, 4]);
});

test("hidden_complete_missing", () => {
  assert.throws(() => complete(demo(), 9), (e) => e instanceof NotFound && e.id === 9 && e instanceof TaskErr);
});

test("hidden_complete_twice", () => {
  assert.throws(
    () => complete(complete(demo(), 2), 2),
    (e) => e instanceof AlreadyDone && e.id === 2 && e instanceof TaskErr,
  );
});

test("hidden_render_high", () => {
  assert.equal(render(task(2, "write report", false, Priority.High)), "[ ] #2 (high) write report");
});

test("hidden_render_low", () => {
  assert.equal(render(task(5, "nap", true, Priority.Low)), "[x] #5 (low) nap");
});

test("hidden_render_all", () => {
  assert.equal(
    renderAll(demo()),
    "[ ] #1 (low) buy milk\n[ ] #2 (high) write report\n[ ] #3 (med) call bob\n[ ] #4 (high) fix sink",
  );
});

test("hidden_by_pri", () => {
  assert.deepEqual(byPriority(demo()).map((t) => t.id), [2, 4, 3, 1]);
});

test("hidden_by_pri_pending", () => {
  assert.deepEqual(byPriority(complete(demo(), 4)).map((t) => t.id), [2, 3, 1]);
});

test("hidden_clear_done", () => {
  assert.deepEqual(clearDone(complete(complete(demo(), 1), 3)).map((t) => t.id), [2, 4]);
});

test("hidden_retitle", () => {
  assert.deepEqual(retitle(demo(), 3, "call alice").map((t) => t.title), ["buy milk", "write report", "call alice", "fix sink"]);
});

test("hidden_retitle_missing", () => {
  assert.throws(() => retitle(demo(), 7, "x"), (e) => e instanceof NotFound && e.id === 7);
});
