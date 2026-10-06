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
    assert average(Student("e", [])) == 0
    assert report_card(Student("e", [])) == "e: 0 (F)"


def test_letter_inclusive():
    assert [letter(x) for x in (90, 80, 70, 60, 59, 100)] == ["A", "B", "C", "D", "F", "A"]
    assert letter(89) == "B"


def test_rank_ties_by_name():
    ss = [Student("c", [80]), Student("a", [80]), Student("b", [90])]
    assert ranking(ss) == ["b", "a", "c"]


def test_drop_lowest():
    assert drop_lowest(Student("x", [5, 3, 9, 3])).scores == [5, 9, 3]
    one = Student("y", [7])
    assert drop_lowest(one) == one
    assert drop_lowest(Student("z", [])).scores == []
    orig = Student("w", [1, 2])
    drop_lowest(orig)
    assert orig.scores == [1, 2]


def test_curve():
    ss = [Student("a", [95, 50]), Student("b", [])]
    out = curve(ss, 10)
    assert [s.scores for s in out] == [[100, 60], []]
    assert ss[0].scores == [95, 50]
    assert curve(ss, 0)[0].scores == [95, 50]


def test_honor_roll():
    assert honor_roll(roster()) == ["bo", "mia", "zed"]
    assert honor_roll([Student("q", [90]), Student("p", [89, 50]), Student("e", [])]) == ["q"]
