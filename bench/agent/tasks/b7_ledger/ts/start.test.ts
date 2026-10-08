import { test } from "node:test";
import assert from "node:assert/strict";
import { UnknownAccount, addTxn, balance, newLedger, p, sample, trialBalance, txn } from "./app";

test("bank", () => {
  assert.equal(balance(sample(), "assets:bank"), 970);
});

test("unknown", () => {
  assert.throws(
    () => addTxn(newLedger(), txn(1, "x", [p("assets:gold", 5), p("equity:owner", -5)])),
    (e) => e instanceof UnknownAccount && e.name === "assets:gold",
  );
});

test("trial sums to zero", () => {
  assert.equal(trialBalance(sample()).reduce((s, [, b]) => s + b, 0), 0);
});
