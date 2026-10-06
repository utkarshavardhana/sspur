from dataclasses import dataclass
from typing import List


@dataclass(frozen=True)
class Student:
    name: str
    scores: List[int]


def average(s: Student) -> int:
    if len(s.scores) == 0:
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


def report_card(s: Student) -> str:
    return f"{s.name}: {average(s)} ({grade(s)})"


def drop_lowest(s: Student) -> Student:
    if len(s.scores) < 2:
        return s
    scores_copy = s.scores.copy()
    min_score = min(scores_copy)
    scores_copy.remove(min_score)
    return Student(s.name, scores_copy)


def curve(ss: List[Student], pts: int) -> List[Student]:
    result = []
    for s in ss:
        curved_scores = [min(score + pts, 100) for score in s.scores]
        result.append(Student(s.name, curved_scores))
    return result


def honor_roll(ss: List[Student]) -> List[str]:
    names = [s.name for s in ss if grade(drop_lowest(s)) == "A"]
    return sorted(names)


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


def test_average_no_scores():
    assert average(Student("x", [])) == 0


def test_letter_inclusive():
    assert letter(90) == "A"
    assert letter(80) == "B"
    assert letter(70) == "C"
    assert letter(60) == "D"


def test_ranking_tie_break():
    ss = [
        Student("bob", [80, 80]),
        Student("alice", [80, 80]),
        Student("charlie", [90, 90]),
    ]
    assert ranking(ss) == ["charlie", "alice", "bob"]


def test_drop_lowest_basic():
    s = Student("x", [50, 60, 70])
    result = drop_lowest(s)
    assert result.scores == [60, 70]


def test_drop_lowest_one_score():
    s = Student("x", [50])
    assert drop_lowest(s) == s


def test_drop_lowest_no_scores():
    s = Student("x", [])
    assert drop_lowest(s) == s


def test_drop_lowest_removes_first_occurrence():
    s = Student("x", [50, 70, 50])
    result = drop_lowest(s)
    assert result.scores == [70, 50]


def test_curve_basic():
    ss = [Student("x", [80, 90])]
    result = curve(ss, 10)
    assert result[0].scores == [90, 100]


def test_curve_cap_at_100():
    ss = [Student("x", [95, 98])]
    result = curve(ss, 10)
    assert result[0].scores == [100, 100]


def test_curve_zero_points():
    ss = [Student("x", [80, 90])]
    result = curve(ss, 0)
    assert result[0].scores == [80, 90]


def test_honor_roll():
    ss = [
        Student("zed", [95, 90, 50]),
        Student("alice", [85, 85, 85]),
        Student("bob", [92, 91, 89]),
    ]
    result = honor_roll(ss)
    assert set(result) >= {"zed", "bob"}
    assert result == sorted(result)
