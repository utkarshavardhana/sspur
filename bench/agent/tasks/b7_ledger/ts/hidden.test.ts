import { test } from "node:test";
import assert from "node:assert/strict";
import {
  DuplicateId,
  Ledger,
  LedgerErr,
  Overdrawn,
  Posting,
  TooFewPostings,
  Unbalanced,
  UnknownAccount,
  UnknownTxn,
  addTxn,
  balance,
  history,
  newLedger,
  p,
  reverse,
  sample,
  trialBalance,
  txn,
} from "./app";

function hAdd(l: Ledger, id: number, ps: readonly Posting[]): string {
  try {
    return `ok ${addTxn(l, txn(id, "t", ps)).txns.length}`;
  } catch (e) {
    if (e instanceof UnknownAccount) return `unknown ${e.name}`;
    if (e instanceof Unbalanced) return `unbalanced ${e.id} ${e.diff}`;
    if (e instanceof TooFewPostings) return `few ${e.id}`;
    if (e instanceof DuplicateId) return `dup ${e.id}`;
    if (e instanceof Overdrawn) return `overdrawn ${e.account} ${e.balance}`;
    if (e instanceof LedgerErr) return "other";
    throw e;
  }
}

function hSampleAdd(id: number, ps: readonly Posting[]): string {
  return hAdd(sample(), id, ps);
}

function hRev(id: number, newId: number): string {
  try {
    return `ok ${balance(reverse(sample(), id, newId), "assets:bank")}`;
  } catch (e) {
    if (e instanceof UnknownTxn) return `no txn ${e.id}`;
    if (e instanceof DuplicateId) return `dup ${e.id}`;
    if (e instanceof Overdrawn) return `overdrawn ${e.account} ${e.balance}`;
    if (e instanceof LedgerErr) return "other";
    throw e;
  }
}

test("hidden_sample", () => {
  assert.equal(balance(sample(), "assets:bank"), 970);
  assert.equal(sample().txns.length, 2);
});

test("hidden_unbalanced", () => {
  assert.equal(hAdd(newLedger(), 1, [p("assets:bank", 10), p("equity:owner", -9)]), "unbalanced 1 1");
  assert.equal(hAdd(newLedger(), 4, [p("assets:bank", 5), p("equity:owner", -8)]), "unbalanced 4 -3");
});

test("hidden_too_few", () => {
  assert.equal(hAdd(newLedger(), 1, [p("assets:bank", 0)]), "few 1");
  assert.equal(hAdd(newLedger(), 2, []), "few 2");
});

test("hidden_unknown", () => {
  assert.equal(hAdd(newLedger(), 1, [p("assets:nope", 5), p("equity:owner", -5)]), "unknown assets:nope");
});

test("hidden_check_order", () => {
  assert.equal(hAdd(newLedger(), 1, [p("zzz", 5)]), "few 1");
  assert.equal(hAdd(newLedger(), 1, [p("zzz", 5), p("equity:owner", -1)]), "unknown zzz");
});

test("hidden_duplicate", () => {
  assert.equal(hSampleAdd(2, [p("expenses:food", 5), p("assets:bank", -5)]), "dup 2");
});

test("hidden_overdrawn", () => {
  assert.equal(hSampleAdd(3, [p("expenses:food", 2000), p("assets:bank", -2000)]), "overdrawn assets:bank -1030");
});

test("hidden_overdrawn_cash", () => {
  assert.equal(hSampleAdd(3, [p("assets:cash", -1), p("assets:bank", 1)]), "overdrawn assets:cash -1");
});

test("hidden_exact_zero", () => {
  assert.equal(hSampleAdd(3, [p("expenses:food", 970), p("assets:bank", -970)]), "ok 3");
});

test("hidden_other_negative", () => {
  assert.equal(hSampleAdd(3, [p("assets:cash", 50), p("income:salary", -50)]), "ok 3");
});

test("hidden_trial", () => {
  assert.deepEqual(trialBalance(sample()), [
    ["assets:bank", 970],
    ["equity:owner", -1000],
    ["expenses:food", 30],
  ]);
});

test("hidden_trial_empty", () => {
  assert.deepEqual(trialBalance(newLedger()), []);
});

test("hidden_reverse", () => {
  assert.equal(hRev(2, 3), "ok 1000");
});

test("hidden_reverse_txn", () => {
  const l = reverse(sample(), 2, 3);
  assert.deepEqual(l.txns.at(-1), txn(3, "reverse 2", [p("expenses:food", -30), p("assets:bank", 30)]));
  assert.deepEqual(trialBalance(l), [
    ["assets:bank", 1000],
    ["equity:owner", -1000],
  ]);
});

test("hidden_reverse_errors", () => {
  assert.equal(hRev(9, 3), "no txn 9");
  assert.equal(hRev(2, 1), "dup 1");
  assert.equal(hRev(1, 3), "overdrawn assets:bank -30");
});

test("hidden_history", () => {
  assert.deepEqual(history(sample(), "assets:bank"), [
    [1, 1000],
    [2, 970],
  ]);
  assert.deepEqual(history(sample(), "expenses:food"), [[2, 30]]);
  assert.deepEqual(history(sample(), "assets:cash"), []);
});

test("hidden_history_net", () => {
  const l = addTxn(sample(), txn(7, "x", [p("assets:bank", 5), p("assets:bank", 5), p("equity:owner", -10)]));
  assert.deepEqual(history(l, "assets:bank"), [
    [1, 1000],
    [2, 970],
    [7, 980],
  ]);
});
