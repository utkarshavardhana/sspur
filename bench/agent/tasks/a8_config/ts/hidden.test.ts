import { test } from "node:test";
import assert from "node:assert/strict";
import { CfgErr, Entry, Missing, NotBool, NotInt, getBool, getInt, keys, lookup, parse } from "./app";

function cfg(): Entry[] {
  return parse(" host = example.com ; port=8080;# port=9;url=http://x?a=b;empty=;=nokey;junk;port = 9090;debug=Yes;n=-5;bad=12a;off=NO");
}

test("hidden_len", () => {
  assert.equal(cfg().length, 9);
});

test("hidden_keys", () => {
  assert.deepEqual(keys(cfg()), ["host", "port", "url", "empty", "debug", "n", "bad", "off"]);
});

test("hidden_trim", () => {
  assert.equal(lookup(cfg(), "host"), "example.com");
});

test("hidden_first_eq", () => {
  assert.equal(lookup(cfg(), "url"), "http://x?a=b");
});

test("hidden_empty_value", () => {
  assert.equal(lookup(cfg(), "empty"), "");
});

test("hidden_last_wins", () => {
  assert.equal(getInt(cfg(), "port"), 9090);
});

test("hidden_neg", () => {
  assert.equal(getInt(cfg(), "n"), -5);
});

test("hidden_not_int", () => {
  assert.throws(
    () => getInt(cfg(), "bad"),
    (e) => e instanceof NotInt && e.key === "bad" && e.value === "12a" && e instanceof CfgErr,
  );
});

test("hidden_int_missing", () => {
  assert.throws(() => getInt(cfg(), "zz"), (e) => e instanceof Missing && e.key === "zz");
});

test("hidden_bool", () => {
  assert.equal(getBool(cfg(), "debug", false), true);
  assert.equal(getBool(cfg(), "off", true), false);
});

test("hidden_bool_default", () => {
  assert.equal(getBool(cfg(), "zz", true), true);
  assert.equal(getBool(cfg(), "zz", false), false);
});

test("hidden_not_bool", () => {
  assert.throws(
    () => getBool(cfg(), "host", false),
    (e) => e instanceof NotBool && e.key === "host" && e.value === "example.com" && e instanceof CfgErr,
  );
});

test("hidden_blank", () => {
  assert.deepEqual(parse("a=1;;b=2; ;#c=3").map((e) => e.key), ["a", "b"]);
});
