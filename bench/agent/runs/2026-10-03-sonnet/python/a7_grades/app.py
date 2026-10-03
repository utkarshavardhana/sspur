from dataclasses import dataclass
from typing import List


@dataclass(frozen=True)
class Student:
    name: str
    scores: List[int]


def average(s: Student) -> int:
    if not s.scores:
        return 0
    return sum(s.scores) // len(s.scores)


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
    scores = list(s.scores)
    scores.remove(min(scores))
    return Student(s.name, scores)


def curve(ss: List[Student], pts: int) -> List[Student]:
    return [Student(s.name, [min(100, x + pts) for x in s.scores]) for s in ss]


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


def test_average_empty():
    assert average(Student("x", [])) == 0


def test_letter_boundaries():
    assert [letter(x) for x in (90, 80, 70, 60, 59)] == ["A", "B", "C", "D", "F"]


def test_rank_tie_by_name():
    ss = [Student("b", [80]), Student("a", [80]), Student("c", [90])]
    assert ranking(ss) == ["c", "a", "b"]


def test_drop_lowest():
    assert drop_lowest(Student("x", [5, 3, 9, 3])).scores == [5, 9, 3]
    s = Student("x", [4])
    assert drop_lowest(s) is s
    assert drop_lowest(Student("x", [])).scores == []


def test_curve():
    out = curve([Student("x", [95, 50])], 10)
    assert out == [Student("x", [100, 60])]


def test_honor_roll():
    ss = [Student("zed", [95, 90, 50]), Student("amy", [90, 90]), Student("bo", [80, 70])]
    assert honor_roll(ss) == ["amy", "zed"]
