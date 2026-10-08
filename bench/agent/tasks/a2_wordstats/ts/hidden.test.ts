import { test } from "node:test";
import assert from "node:assert/strict";
import { longestWord, summary, topK, topLine, wordFreq, wordsOf } from "./app";

const TEXT = "the cat and the dog and the bird";

test("hidden_ties", () => {
  assert.deepEqual(topK(TEXT, 3, []), [["the", 3], ["and", 2], ["bird", 1]]);
});

test("hidden_ties_all", () => {
  assert.deepEqual(topK("d c b a c", 4, []), [["c", 2], ["a", 1], ["b", 1], ["d", 1]]);
});

test("hidden_stop_top", () => {
  assert.deepEqual(topK(TEXT, 2, ["the", "and"]), [["bird", 1], ["cat", 1]]);
});

test("hidden_stop_freq", () => {
  assert.deepEqual(wordFreq("The THE cat", ["the"]), [["cat", 1]]);
});

test("hidden_words_stop", () => {
  assert.deepEqual(wordsOf("A b, a C!", ["a"]), ["b", "c"]);
});

test("hidden_summary_stop", () => {
  assert.equal(summary(TEXT, ["the"]), "5 words, 4 distinct, top: and=2 bird=1 cat=1");
});

test("hidden_summary_nostop", () => {
  assert.equal(summary("b a b", []), "3 words, 2 distinct, top: b=2 a=1");
});

test("hidden_top_line", () => {
  assert.equal(topLine("q q r", 5, ["r"]), "q=2");
});

test("hidden_longest", () => {
  assert.equal(longestWord("a bb ccc dd eee"), "ccc");
});

test("hidden_longest_case", () => {
  assert.equal(longestWord("Hello World"), "hello");
});

test("hidden_longest_none", () => {
  assert.equal(longestWord("!! ??"), undefined);
});
