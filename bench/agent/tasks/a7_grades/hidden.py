from app import *


def test_hidden_empty():
    assert average(Student("e", [])) == 0
    assert report_card(Student("e", [])) == "e: 0 (F)"


def test_hidden_bounds():
    assert [letter(x) for x in (90, 89, 80, 70, 60, 59, 100)] == ["A", "B", "B", "C", "D", "F", "A"]


def test_hidden_rank():
    assert ranking(roster()) == ["bo", "mia", "zed", "ali"]


def test_hidden_rank_ties():
    assert ranking([Student("cy", [80]), Student("al", [80]), Student("bo", [90])]) == ["bo", "al", "cy"]


def test_hidden_drop():
    assert drop_lowest(Student("z", [95, 50, 90, 50])).scores == [95, 90, 50]


def test_hidden_drop_one():
    assert drop_lowest(Student("z", [70])).scores == [70]
    assert drop_lowest(Student("z", [])).scores == []


def test_hidden_drop_name():
    assert drop_lowest(Student("q", [1, 2])) == Student("q", [2])


def test_hidden_curve():
    assert curve([Student("a", [95, 80]), Student("b", [])], 10) == [Student("a", [100, 90]), Student("b", [])]


def test_hidden_honor():
    assert honor_roll(roster()) == ["bo", "mia", "zed"]


def test_hidden_honor_empty():
    assert honor_roll([]) == []


def test_hidden_grade():
    assert grade(Student("m", [90, 90])) == "A"
