from dataclasses import dataclass
from typing import List


@dataclass(frozen=True)
class Student:
    name: str
    scores: List[int]


def average(s: Student) -> int:
    return sum(s.scores) // len(s.scores) if s.scores else 0


def letter(avg: int) -> str:
    if avg >= 90:
        return "A"
    if avg >= 80:
        return "B"
    if avg >= 70:
        return "C"
    if avg >= 60:
        return "D"
    return "F"


def grade(s: Student) -> str:
    return letter(average(s))


def ranking(ss: List[Student]) -> List[str]:
    return [s.name for s in sorted(ss, key=lambda s: (-average(s), s.name))]


def drop_lowest(s: Student) -> Student:
    if len(s.scores) < 2:
        return s
    rest = list(s.scores)
    rest.remove(min(rest))
    return Student(s.name, rest)


def curve(ss: List[Student], pts: int) -> List[Student]:
    return [Student(s.name, [min(x + pts, 100) for x in s.scores]) for s in ss]


def honor_roll(ss: List[Student]) -> List[str]:
    return sorted(s.name for s in ss if grade(drop_lowest(s)) == "A")


def report_card(s: Student) -> str:
    return f"{s.name}: {average(s)} ({grade(s)})"


def roster() -> List[Student]:
    return [
        Student("mia", [88, 92, 95]),
        Student("ali", [70, 65, 80]),
        Student("zed", [95, 90, 50]),
        Student("bo", [100, 92, 89]),
    ]


if __name__ == "__main__":
    print("\n".join(report_card(s) for s in roster()))


def test_card():
    assert report_card(Student("x", [50, 60])) == "x: 55 (F)"


def test_rank_order():
    assert ranking(roster())[:2] == ["bo", "mia"]


def test_letter_b():
    assert letter(85) == "B"
