import { test } from "node:test";
import assert from "node:assert/strict";
import { BadChar, BadSyntax, CalcErr, DivByZero, calc, showResult } from "./app";

function hRes(s: string): string {
  try {
    return String(calc(s));
  } catch (e) {
    if (e instanceof DivByZero) return "div";
    if (e instanceof BadSyntax) return "syntax";
    if (e instanceof BadChar) return "char " + e.ch;
    if (e instanceof CalcErr) return "other";
    throw e;
  }
}

test("hidden_precedence", () => {
  assert.equal(hRes("1 + 2 * 3"), "7");
  assert.equal(hRes("2 * 3 + 4"), "10");
  assert.equal(hRes("20 - 6 / 3"), "18");
});

test("hidden_left_assoc", () => {
  assert.equal(hRes("8 - 3 - 2"), "3");
  assert.equal(hRes("16 / 4 / 2"), "2");
  assert.equal(hRes("2 * 3 / 4"), "1");
});

test("hidden_truncate", () => {
  assert.equal(hRes("7 / 2"), "3");
  assert.equal(hRes("-7 / 2"), "-3");
  assert.equal(hRes("7 / -2"), "-3");
});

test("hidden_no_spaces", () => {
  assert.equal(hRes("12*3-4"), "32");
});

test("hidden_div_zero", () => {
  assert.equal(hRes("1 / 0"), "div");
  assert.equal(hRes("5 / (2 - 2)"), "div");
});

test("hidden_parens", () => {
  assert.equal(hRes("2 * (3 + 4)"), "14");
  assert.equal(hRes("((2))"), "2");
  assert.equal(hRes("(1 + 2) * (3 + 4)"), "21");
  assert.equal(hRes("10 - (2 - 3)"), "11");
});

test("hidden_nested_parens", () => {
  assert.equal(hRes("2 * (3 + (4 - 1) * 2)"), "18");
});

test("hidden_unary", () => {
  assert.equal(hRes("-3"), "-3");
  assert.equal(hRes("2 * -3"), "-6");
  assert.equal(hRes("-(1 + 2)"), "-3");
  assert.equal(hRes("2 - -3"), "5");
  assert.equal(hRes("(-4) * 2"), "-8");
});

test("hidden_empty", () => {
  assert.equal(hRes(""), "syntax");
  assert.equal(hRes("   "), "syntax");
});

test("hidden_missing_operand", () => {
  assert.equal(hRes("1 +"), "syntax");
  assert.equal(hRes("* 2"), "syntax");
});

test("hidden_two_numbers", () => {
  assert.equal(hRes("1 2"), "syntax");
});

test("hidden_unbalanced", () => {
  assert.equal(hRes("(1 + 2"), "syntax");
  assert.equal(hRes("1 + 2)"), "syntax");
});

test("hidden_empty_parens", () => {
  assert.equal(hRes("()"), "syntax");
});

test("hidden_bad_char", () => {
  assert.equal(hRes("1 $ 2"), "char $");
});

test("hidden_show_div", () => {
  assert.equal(showResult("1 / 0"), "1 / 0 = error: division by zero");
});

test("hidden_show_syntax", () => {
  assert.equal(showResult("1 +"), "1 + = error: syntax");
});

test("hidden_show_ok", () => {
  assert.equal(showResult("2 * (3 + 4)"), "2 * (3 + 4) = 14");
  assert.equal(showResult("1 ? 2"), "1 ? 2 = error: bad character ?");
});
