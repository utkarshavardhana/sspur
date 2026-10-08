import { test } from "node:test";
import assert from "node:assert/strict";
import { BankErr, Frozen, UnknownAccount, account, accounts, attempt, deposit, freeze, openAccount, withdraw } from "./app";

test("hidden_no_fee", () => {
  assert.equal(attempt(accounts(), "a1", "b1", 60), "alice: 40, bob: 80, carol: 0");
});

test("hidden_fee", () => {
  assert.equal(attempt(accounts(), "a1", "b1", 10), "alice: 89, bob: 30, carol: 0");
});

test("hidden_fee_short", () => {
  assert.equal(attempt(accounts(), "b1", "a1", 20), "declined: b1 needs 21, has 20");
});

test("hidden_fee_exact", () => {
  assert.equal(attempt(accounts(), "b1", "a1", 19), "alice: 119, bob: 0, carol: 0");
});

test("hidden_frozen_to", () => {
  assert.equal(attempt(freeze(accounts(), "b1"), "a1", "b1", 60), "declined: b1 is frozen");
});

test("hidden_frozen_from", () => {
  assert.equal(attempt(freeze(accounts(), "a1"), "a1", "b1", 60), "declined: a1 is frozen");
});

test("hidden_frozen_other", () => {
  assert.equal(attempt(freeze(accounts(), "c1"), "a1", "b1", 60), "alice: 40, bob: 80, carol: 0");
});

test("hidden_deposit_frozen", () => {
  assert.throws(
    () => deposit(freeze(accounts(), "c1"), "c1", 5),
    (e) => e instanceof Frozen && e.id === "c1" && e instanceof BankErr,
  );
});

test("hidden_withdraw_frozen", () => {
  assert.throws(() => withdraw([account("z", "zed", 5, true)], "z", 1), (e) => e instanceof Frozen && e.id === "z");
});

test("hidden_freeze_unknown", () => {
  assert.throws(() => freeze(accounts(), "q"), (e) => e instanceof UnknownAccount && e.id === "q");
});

test("hidden_freeze_sets", () => {
  assert.deepEqual(freeze(accounts(), "b1").map((a) => a.frozen), [false, true, false]);
});

test("hidden_unfrozen", () => {
  assert.ok(accounts().every((a) => !a.frozen));
  const bank = openAccount(accounts(), "d1", "dan");
  assert.equal(bank[bank.length - 1].frozen, false);
});

test("hidden_same", () => {
  assert.equal(attempt(accounts(), "a1", "a1", 5), "declined: same account");
});
