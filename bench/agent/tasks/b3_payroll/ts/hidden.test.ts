import { test } from "node:test";
import assert from "node:assert/strict";
import { BadNumber, BadRow, CsvErr, parse, pay, report, row, sample, validate } from "./app";

function hErr(text: string): string {
  try {
    return `rows ${parse(text).length}`;
  } catch (e) {
    if (e instanceof BadRow) return `row ${e.line}`;
    if (e instanceof BadNumber) return `number ${e.line} ${e.field}`;
    if (e instanceof CsvErr) return "other";
    throw e;
  }
}

test("hidden_sample", () => {
  assert.equal(parse(sample()).length, 6);
  assert.equal(pay(parse(sample())[2]), 4000);
});

test("hidden_header", () => {
  assert.equal(hErr("name,rate,hours\nBeth,400,10"), "rows 1");
});

test("hidden_header_trimmed", () => {
  assert.equal(hErr("  name,rate,hours  \nBeth,400,10"), "rows 1");
});

test("hidden_header_not_first", () => {
  assert.equal(hErr("Beth,400,10\nname,rate,hours"), "number 2 rate");
});

test("hidden_blank_lines", () => {
  assert.equal(hErr("\nBeth,400,10\n\n   \nDan,375,2\n"), "rows 2");
  assert.equal(hErr(""), "rows 0");
});

test("hidden_trims", () => {
  assert.deepEqual(parse(" Beth , 400 , 10 "), [row("Beth", 400, 10)]);
});

test("hidden_bad_row", () => {
  assert.equal(hErr("Beth,400\nDan,375,2"), "row 1");
  assert.equal(hErr("a,1,2\nb,1,2,3"), "row 2");
});

test("hidden_line_numbers", () => {
  assert.equal(hErr("name,rate,hours\n\nBeth,4x,1"), "number 3 rate");
});

test("hidden_bad_hours", () => {
  assert.equal(hErr("Beth,400,ten"), "number 1 hours");
  assert.equal(hErr("Beth,400,-5"), "number 1 hours");
});

test("hidden_rate_first", () => {
  assert.equal(hErr("Beth,x,y"), "number 1 rate");
  assert.equal(hErr("Beth,-1,3"), "number 1 rate");
});

test("hidden_overtime", () => {
  assert.equal(pay(row("x", 400, 45)), 19000);
  assert.equal(pay(row("y", 5, 41)), 207);
});

test("hidden_no_overtime", () => {
  assert.equal(pay(row("x", 400, 40)), 16000);
  assert.equal(pay(row("z", 375, 0)), 0);
});

test("hidden_report_sorted", () => {
  assert.equal(report(parse("Beth,400,0\nDan,375,0\nKathy,400,10")), "Kathy: 4000\nBeth: 0\nDan: 0\ntotal: 4000");
});

test("hidden_report_overtime", () => {
  assert.equal(report([row("a", 10, 50), row("b", 20, 30)]), "b: 600\na: 550\ntotal: 1150");
});

test("hidden_report_empty", () => {
  assert.equal(report([]), "total: 0");
});

test("hidden_validate", () => {
  assert.deepEqual(validate("name,rate,hours\nBeth,400\nDan,x,1\nKathy,400,10\nMark,500,z"), [
    "line 2: expected 3 fields",
    "line 3: bad rate",
    "line 5: bad hours",
  ]);
});

test("hidden_validate_ok", () => {
  assert.deepEqual(validate(sample()), []);
  assert.deepEqual(validate(""), []);
});
