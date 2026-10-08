import { test } from "node:test";
import assert from "node:assert/strict";
import { Cycle, DepErr, Missing, Pkg, Stuck, Unknown, dependents, fullOrder, installOrder, pkg, plan, registry } from "./app";

function hOrder(reg: readonly Pkg[], target: string): string {
  try {
    return installOrder(reg, target).join(",");
  } catch (e) {
    if (e instanceof Unknown) return `unknown ${e.name}`;
    if (e instanceof Missing) return `missing ${e.pkg} ${e.dep}`;
    if (e instanceof Cycle) return "cycle " + e.path.join(",");
    if (e instanceof DepErr) return "other";
    throw e;
  }
}

function hFull(reg: readonly Pkg[]): string {
  try {
    return fullOrder(reg).join(",");
  } catch (e) {
    if (e instanceof Stuck) return "stuck " + e.names.join(",");
    if (e instanceof DepErr) return "other";
    throw e;
  }
}

function hDeps(reg: readonly Pkg[], name: string): string {
  try {
    return dependents(reg, name).join(",");
  } catch (e) {
    if (e instanceof Unknown) return `unknown ${e.name}`;
    if (e instanceof DepErr) return "other";
    throw e;
  }
}

function hDiamond(): Pkg[] {
  return [pkg("a", ["b", "c"]), pkg("b", ["d"]), pkg("c", ["d"]), pkg("d", [])];
}

test("hidden_once", () => {
  assert.equal(hOrder(registry(), "app"), "http,log,web,db,app");
});

test("hidden_leaf", () => {
  assert.equal(hOrder(registry(), "log"), "log");
});

test("hidden_diamond", () => {
  assert.equal(hOrder(hDiamond(), "a"), "d,b,c,a");
});

test("hidden_unknown", () => {
  assert.equal(hOrder(registry(), "zzz"), "unknown zzz");
});

test("hidden_missing", () => {
  assert.equal(hOrder([pkg("a", ["b"]), pkg("b", ["x"])], "a"), "missing b x");
});

test("hidden_cycle", () => {
  assert.equal(hOrder([pkg("a", ["b"]), pkg("b", ["c"]), pkg("c", ["a"])], "a"), "cycle a,b,c,a");
});

test("hidden_self_cycle", () => {
  assert.equal(hOrder([pkg("a", ["a"])], "a"), "cycle a,a");
});

test("hidden_deep_cycle", () => {
  assert.equal(hOrder([pkg("top", ["b"]), pkg("b", ["c"]), pkg("c", ["b"])], "top"), "cycle b,c,b");
});

test("hidden_full", () => {
  assert.equal(hFull(registry()), "http,log,db,web,app");
});

test("hidden_full_diamond", () => {
  assert.equal(hFull(hDiamond()), "d,b,c,a");
});

test("hidden_full_stuck", () => {
  assert.equal(hFull([pkg("a", ["b"]), pkg("b", ["a"]), pkg("c", [])]), "stuck a,b");
});

test("hidden_full_missing", () => {
  assert.equal(hFull([pkg("a", ["x"]), pkg("b", [])]), "stuck a");
});

test("hidden_full_empty", () => {
  assert.equal(hFull([]), "");
});

test("hidden_dependents", () => {
  assert.equal(hDeps(registry(), "log"), "app,db,web");
  assert.equal(hDeps(registry(), "http"), "app,web");
});

test("hidden_dependents_none", () => {
  assert.equal(hDeps(registry(), "app"), "");
  assert.equal(hDeps(hDiamond(), "d"), "a,b,c");
});

test("hidden_dependents_unknown", () => {
  assert.equal(hDeps(registry(), "zzz"), "unknown zzz");
});

test("hidden_plan", () => {
  assert.equal(plan(registry(), "app"), "http -> log -> web -> db -> app");
});

test("hidden_plan_errors", () => {
  assert.equal(plan([pkg("a", ["b"]), pkg("b", ["x"])], "a"), "error: b needs missing x");
  assert.equal(plan([pkg("a", ["b"]), pkg("b", ["a"])], "a"), "error: cycle a -> b -> a");
});
