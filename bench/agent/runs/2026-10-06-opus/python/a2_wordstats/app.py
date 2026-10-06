import re
from collections import Counter
from typing import List, Optional, Tuple


def words_of(text: str, stop: List[str]) -> List[str]:
    excluded = set(stop)
    return [w for w in re.findall(r"[a-z0-9]+", text.lower()) if w not in excluded]


def word_freq(text: str, stop: List[str]) -> List[Tuple[str, int]]:
    return list(Counter(words_of(text, stop)).items())


def top_k(text: str, k: int, stop: List[str]) -> List[Tuple[str, int]]:
    assert k >= 0
    return sorted(word_freq(text, stop), key=lambda p: (-p[1], p[0]))[:k]


def fmt_pair(p: Tuple[str, int]) -> str:
    return f"{p[0]}={p[1]}"


def top_line(text: str, k: int, stop: List[str]) -> str:
    return " ".join(fmt_pair(p) for p in top_k(text, k, stop))


def summary(text: str, stop: List[str]) -> str:
    n = len(words_of(text, stop))
    d = len(word_freq(text, stop))
    return f"{n} words, {d} distinct, top: {top_line(text, 3, stop)}"


def longest_word(text: str) -> Optional[str]:
    best: Optional[str] = None
    for w in words_of(text, []):
        if best is None or len(w) > len(best):
            best = w
    return best


if __name__ == "__main__":
    print(summary("The cat and the hat. The end!", []))


def test_freq_basic():
    assert word_freq("a b a", []) == [("a", 2), ("b", 1)]


def test_top_one():
    assert top_k("x y y z", 1, []) == [("y", 2)]


def test_summary_basic():
    assert summary("b a b", []) == "3 words, 2 distinct, top: b=2 a=1"


def test_top_k_ties_alphabetical():
    assert top_k("c b a c b a d", 4, []) == [("a", 2), ("b", 2), ("c", 2), ("d", 1)]
    assert top_line("z y x", 2, []) == "x=1 y=1"


def test_stop_words_excluded():
    assert words_of("The cat THE hat", ["the"]) == ["cat", "hat"]
    assert word_freq("a b a c", ["a"]) == [("b", 1), ("c", 1)]
    assert top_k("the the the cat", 1, ["the"]) == [("cat", 1)]
    assert top_line("The cat and the hat", 3, ["the", "and"]) == "cat=1 hat=1"


def test_summary_with_stop():
    assert summary("The cat and the hat. The end!", ["the", "and"]) == (
        "3 words, 3 distinct, top: cat=1 end=1 hat=1"
    )
    assert summary("the THE", ["the"]) == "0 words, 0 distinct, top: "


def test_longest_word():
    assert longest_word("a bbb cc ddd") == "bbb"
    assert longest_word("Hello, World!!") == "hello"
    assert longest_word("") is None
    assert longest_word("!!! ...") is None
