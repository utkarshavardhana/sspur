import { test } from "node:test";
import assert from "node:assert/strict";
import { defaults, getPath, merge, render } from "./app";

test("renders", () => {
  assert.equal(render({ a: [1, true, null] }), '{"a":[1,true,null]}');
});

test("path", () => {
  assert.equal(getPath(defaults(), "db.pool"), 5);
  assert.equal(getPath(defaults(), "db.user"), undefined);
});

test("merges top level", () => {
  assert.equal(getPath(merge(defaults(), { name: "web" }), "name"), "web");
});
