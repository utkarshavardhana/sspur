import pytest
from app import *


def test_hidden_complete_ok():
    assert [t.id for t in pending(complete(demo(), 2))] == [1, 3, 4]


def test_hidden_complete_missing():
    with pytest.raises(NotFound) as e:
        complete(demo(), 9)
    assert e.value.id == 9
    assert isinstance(e.value, TaskErr)


def test_hidden_complete_twice():
    with pytest.raises(AlreadyDone) as e:
        complete(complete(demo(), 2), 2)
    assert e.value.id == 2
    assert isinstance(e.value, TaskErr)


def test_hidden_render_high():
    assert render(Task(2, "write report", False, Priority.HIGH)) == "[ ] #2 (high) write report"


def test_hidden_render_low():
    assert render(Task(5, "nap", True, Priority.LOW)) == "[x] #5 (low) nap"


def test_hidden_render_all():
    assert render_all(demo()) == "[ ] #1 (low) buy milk\n[ ] #2 (high) write report\n[ ] #3 (med) call bob\n[ ] #4 (high) fix sink"


def test_hidden_by_pri():
    assert [t.id for t in by_priority(demo())] == [2, 4, 3, 1]


def test_hidden_by_pri_pending():
    assert [t.id for t in by_priority(complete(demo(), 4))] == [2, 3, 1]


def test_hidden_clear_done():
    assert [t.id for t in clear_done(complete(complete(demo(), 1), 3))] == [2, 4]


def test_hidden_retitle():
    assert [t.title for t in retitle(demo(), 3, "call alice")] == ["buy milk", "write report", "call alice", "fix sink"]


def test_hidden_retitle_missing():
    with pytest.raises(NotFound) as e:
        retitle(demo(), 7, "x")
    assert e.value.id == 7
