from app import *

TEXT = "the cat and the dog and the bird"


def test_hidden_ties():
    assert top_k(TEXT, 3, []) == [("the", 3), ("and", 2), ("bird", 1)]


def test_hidden_ties_all():
    assert top_k("d c b a c", 4, []) == [("c", 2), ("a", 1), ("b", 1), ("d", 1)]


def test_hidden_stop_top():
    assert top_k(TEXT, 2, ["the", "and"]) == [("bird", 1), ("cat", 1)]


def test_hidden_stop_freq():
    assert word_freq("The THE cat", ["the"]) == [("cat", 1)]


def test_hidden_words_stop():
    assert words_of("A b, a C!", ["a"]) == ["b", "c"]


def test_hidden_summary_stop():
    assert summary(TEXT, ["the"]) == "5 words, 4 distinct, top: and=2 bird=1 cat=1"


def test_hidden_summary_nostop():
    assert summary("b a b", []) == "3 words, 2 distinct, top: b=2 a=1"


def test_hidden_top_line():
    assert top_line("q q r", 5, ["r"]) == "q=2"


def test_hidden_longest():
    assert longest_word("a bb ccc dd eee") == "ccc"


def test_hidden_longest_case():
    assert longest_word("Hello World") == "hello"


def test_hidden_longest_none():
    assert longest_word("!! ??") is None
