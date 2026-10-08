import { test } from "node:test";
import assert from "node:assert/strict";
import { ConfigErr, J, MissingKey, defaults, getPath, merge, mergeAll, render, requirePaths } from "./app";

function hM(t: J, p: J): string {
  return render(merge(t, p));
}

function hCfg(): J {
  return { name: "app", servers: [{ host: "a" }, { host: "b" }], db: { pool: 5 } };
}

function hReq(j: J, paths: readonly string[]): string {
  try {
    requirePaths(j, paths);
    return "ok";
  } catch (e) {
    if (e instanceof MissingKey) return "missing " + e.path;
    if (e instanceof ConfigErr) return "other";
    throw e;
  }
}

test("hidden_rfc_replace", () => {
  assert.equal(hM({ a: "b" }, { a: "c" }), '{"a":"c"}');
});

test("hidden_rfc_add", () => {
  assert.equal(hM({ a: "b" }, { b: "c" }), '{"a":"b","b":"c"}');
});

test("hidden_rfc_remove", () => {
  assert.equal(hM({ a: "b" }, { a: null }), "{}");
  assert.equal(hM({ a: "b", b: "c" }, { a: null }), '{"b":"c"}');
});

test("hidden_rfc_array_replaced", () => {
  assert.equal(hM({ a: ["b"] }, { a: "c" }), '{"a":"c"}');
  assert.equal(hM({ a: "c" }, { a: ["b"] }), '{"a":["b"]}');
});

test("hidden_rfc_nested", () => {
  assert.equal(hM({ a: { b: "c" } }, { a: { b: "d", c: null } }), '{"a":{"b":"d"}}');
});

test("hidden_rfc_array_of_objects", () => {
  assert.equal(hM({ a: [{ b: "c" }] }, { a: [1] }), '{"a":[1]}');
});

test("hidden_rfc_non_object_patch", () => {
  assert.equal(hM(["a", "b"], ["c", "d"]), '["c","d"]');
  assert.equal(hM({ a: "b" }, ["c"]), '["c"]');
  assert.equal(hM({ a: "foo" }, null), "null");
  assert.equal(hM({ a: "foo" }, "bar"), '"bar"');
});

test("hidden_rfc_keep_target_null", () => {
  assert.equal(hM({ e: null }, { a: 1 }), '{"e":null,"a":1}');
});

test("hidden_rfc_non_object_target", () => {
  assert.equal(hM([1, 2], { a: "b", c: null }), '{"a":"b"}');
});

test("hidden_rfc_deep_new", () => {
  assert.equal(hM({}, { a: { bb: { ccc: null } } }), '{"a":{"bb":{}}}');
});

test("hidden_key_order", () => {
  assert.equal(hM({ a: 1, b: 2, c: 3 }, { b: 9, d: 4 }), '{"a":1,"b":9,"c":3,"d":4}');
});

test("hidden_defaults_merge", () => {
  assert.equal(
    hM(defaults(), { port: 9090, db: { pool: 10, user: "u" } }),
    '{"name":"app","port":9090,"db":{"host":"localhost","pool":10,"user":"u"},"tags":["a"]}',
  );
});

test("hidden_escape", () => {
  assert.equal(render('say "hi" \\ bye'), '"say \\"hi\\" \\\\ bye"');
  assert.equal(render({ 'k"q': 1 }), '{"k\\"q":1}');
});

test("hidden_path_index", () => {
  assert.equal(getPath(hCfg(), "servers.1.host"), "b");
  assert.deepEqual(getPath(hCfg(), "servers.0"), { host: "a" });
});

test("hidden_path_missing", () => {
  assert.equal(getPath(hCfg(), "servers.5.host"), undefined);
  assert.equal(getPath(hCfg(), "servers.x"), undefined);
  assert.equal(getPath(hCfg(), "name.first"), undefined);
  assert.equal(getPath(hCfg(), "db.pool"), 5);
});

test("hidden_merge_all", () => {
  assert.equal(render(mergeAll(defaults(), [])), render(defaults()));
  assert.equal(render(mergeAll({ a: 1 }, [{ a: 2, b: 3 }, { b: null }])), '{"a":2}');
});

test("hidden_require", () => {
  assert.equal(hReq(hCfg(), ["name", "db.pool", "servers.1.host"]), "ok");
  assert.equal(hReq(hCfg(), ["name", "db.user", "x"]), "missing db.user");
  assert.equal(hReq(hCfg(), []), "ok");
});
