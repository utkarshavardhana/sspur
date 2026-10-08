import { test } from "node:test";
import assert from "node:assert/strict";
import { Add, CalcErr, Div, DivByZero, Mul, Num, Sub, Var, evalExpr, run, show, simplify, varNames } from "./app";

const noEnv = new Map<string, number>();

test("hidden_sub", () => {
  assert.equal(run(new Sub(new Num(10), new Var("x")), new Map([["x", 3]])), "(10 - x) = 7");
});

test("hidden_div", () => {
  assert.equal(run(new Div(new Num(7), new Num(2)), noEnv), "7 / 2 = 3");
});

test("hidden_div_zero", () => {
  assert.equal(
    run(new Div(new Num(1), new Sub(new Var("x"), new Var("x"))), new Map([["x", 4]])),
    "1 / (x - x) = error: division by zero",
  );
});

test("hidden_unbound_still", () => {
  assert.equal(run(new Sub(new Var("w"), new Num(1)), noEnv), "(w - 1) = error: unbound w");
});

test("hidden_eval_neg", () => {
  assert.equal(evalExpr(new Mul(new Sub(new Num(2), new Num(5)), new Num(3)), noEnv), -9);
});

test("hidden_eval_divzero", () => {
  assert.throws(
    () => evalExpr(new Div(new Num(5), new Num(0)), noEnv),
    (e) => e instanceof DivByZero && e instanceof CalcErr,
  );
});

test("hidden_simp_sub", () => {
  assert.equal(show(simplify(new Sub(new Var("y"), new Num(0)))), "y");
});

test("hidden_simp_div", () => {
  assert.equal(show(simplify(new Div(new Add(new Num(0), new Var("y")), new Num(1)))), "y");
});

test("hidden_simp_inner", () => {
  assert.equal(show(simplify(new Sub(new Mul(new Num(1), new Var("a")), new Var("b")))), "(a - b)");
});

test("hidden_simp_div_inner", () => {
  assert.equal(show(simplify(new Div(new Var("a"), new Add(new Num(2), new Num(0))))), "a / 2");
});

test("hidden_vars", () => {
  assert.deepEqual(
    varNames(new Add(new Var("x"), new Mul(new Var("y"), new Div(new Var("x"), new Var("z"))))),
    ["x", "y", "z"],
  );
});

test("hidden_vars_none", () => {
  assert.deepEqual(varNames(new Sub(new Num(3), new Num(1))), []);
});

test("hidden_old", () => {
  assert.equal(run(new Add(new Num(2), new Mul(new Var("x"), new Num(4))), new Map([["x", 3]])), "(2 + x * 4) = 14");
});
