import { test } from "node:test";
import assert from "node:assert/strict";
import { Add, Mul, Num, Var, run, show, simplify } from "./app";

test("run ok", () => {
  assert.equal(run(new Add(new Num(2), new Mul(new Var("x"), new Num(4))), new Map([["x", 3]])), "(2 + x * 4) = 14");
});

test("run unbound", () => {
  assert.equal(run(new Var("y"), new Map()), "y = error: unbound y");
});

test("simplify ok", () => {
  assert.equal(show(simplify(new Add(new Num(0), new Mul(new Num(1), new Var("z"))))), "z");
});
