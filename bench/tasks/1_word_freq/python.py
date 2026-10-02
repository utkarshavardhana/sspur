import re
from collections import Counter


def top_words(text: str, k: int) -> list[tuple[str, int]]:
    counts = Counter(re.findall(r"[a-z]+", text.lower()))
    return sorted(counts.items(), key=lambda p: (-p[1], p[0]))[:k]
