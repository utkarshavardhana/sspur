import { test } from "node:test";
import assert from "node:assert/strict";
import { accounts, attempt, describe, openAccount } from "./app";

test("big transfer", () => {
  assert.equal(attempt(accounts(), "a1", "b1", 60), "alice: 40, bob: 80, carol: 0");
});

test("declined", () => {
  assert.equal(attempt(accounts(), "b1", "a1", 50), "declined: b1 needs 50, has 20");
});

test("unknown", () => {
  assert.equal(attempt(accounts(), "a1", "zz", 5), "declined: no account zz");
});

test("opened", () => {
  const bank = openAccount(accounts(), "d1", "dan");
  assert.equal(describe(bank[bank.length - 1]), "dan: 0");
});
