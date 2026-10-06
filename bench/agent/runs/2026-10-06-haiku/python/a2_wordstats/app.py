import re
from collections import Counter
from typing import List, Tuple, Optional


def words_of(text: str, stop: List[str]) -> List[str]:
    all_words = re.findall(r"[a-z0-9]+", text.lower())
    stop_set = set(stop)
    return [w for w in all_words if w not in stop_set]


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
    words = words_of(text, [])
    if not words:
        return None
    # Sort by length descending, then by index ascending (first occurrence)
    longest = words[0]
    for word in words:
        if len(word) > len(longest):
            longest = word
    return longest


if __name__ == "__main__":
    print(summary("The cat and the hat. The end!", []))


def test_freq_basic():
    assert word_freq("a b a", []) == [("a", 2), ("b", 1)]


def test_top_one():
    assert top_k("x y y z", 1, []) == [("y", 2)]


def test_summary_basic():
    assert summary("b a b", []) == "3 words, 2 distinct, top: b=2 a=1"


def test_top_k_tie_ordering():
    # Test that ties are broken alphabetically (ascending)
    result = top_k("a b a b c", 3, [])
    assert result == [("a", 2), ("b", 2), ("c", 1)]


def test_stop_words():
    # Test stop words are excluded
    result = words_of("the cat and the dog", ["the", "and"])
    assert result == ["cat", "dog"]


def test_stop_words_in_freq():
    # Test stop words are excluded from word_freq
    result = word_freq("the cat and the dog", ["the", "and"])
    assert result == [("cat", 1), ("dog", 1)] or result == [("dog", 1), ("cat", 1)]


def test_stop_words_in_top_k():
    # Test stop words are excluded from top_k
    result = top_k("a a the the b", 2, ["the"])
    assert result == [("a", 2), ("b", 1)]


def test_stop_words_in_summary():
    # Test stop words are excluded from summary counts
    result = summary("a a the the b", ["the"])
    assert result == "3 words, 2 distinct, top: a=2 b=1"


def test_longest_word_basic():
    # Test longest_word returns the longest word (ties go to first occurrence)
    result = longest_word("apple banana cherry")
    assert result == "banana"


def test_longest_word_tie():
    # Test longest_word breaks ties by first occurrence
    result = longest_word("abc xyz def abc")
    assert result == "abc"


def test_longest_word_no_words():
    # Test longest_word returns None for text with no words
    result = longest_word("!!!")
    assert result is None


def test_longest_word_single():
    # Test longest_word with single word
    result = longest_word("hello")
    assert result == "hello"
