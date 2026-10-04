from dataclasses import dataclass, replace
from enum import Enum
from typing import List


class Priority(Enum):
    LOW = "low"
    MED = "med"
    HIGH = "high"


@dataclass(frozen=True)
class Task:
    id: int
    title: str
    done: bool
    pri: Priority


def next_id(ts: List[Task]) -> int:
    return max((t.id for t in ts), default=0) + 1


def add_task(ts: List[Task], title: str, pri: Priority) -> List[Task]:
    return ts + [Task(next_id(ts), title, False, pri)]


class TaskErr(Exception):
    pass


class NotFound(TaskErr):
    def __init__(self, id: int):
        super().__init__(f"task {id} not found")
        self.id = id


class AlreadyDone(TaskErr):
    def __init__(self, id: int):
        super().__init__(f"task {id} already done")
        self.id = id


def find(ts: List[Task], id: int) -> Task:
    for t in ts:
        if t.id == id:
            return t
    raise NotFound(id)


def complete(ts: List[Task], id: int) -> List[Task]:
    if find(ts, id).done:
        raise AlreadyDone(id)
    return [replace(t, done=True) if t.id == id else t for t in ts]


def retitle(ts: List[Task], id: int, title: str) -> List[Task]:
    find(ts, id)
    return [replace(t, title=title) if t.id == id else t for t in ts]


def clear_done(ts: List[Task]) -> List[Task]:
    return [t for t in ts if not t.done]


_RANK = {Priority.HIGH: 0, Priority.MED: 1, Priority.LOW: 2}


def by_priority(ts: List[Task]) -> List[Task]:
    return sorted(pending(ts), key=lambda t: (_RANK[t.pri], t.id))


def pending(ts: List[Task]) -> List[Task]:
    return [t for t in ts if not t.done]


def box(t: Task) -> str:
    return "[x]" if t.done else "[ ]"


def render(t: Task) -> str:
    return f"{box(t)} #{t.id} ({t.pri.value}) {t.title}"


def render_all(ts: List[Task]) -> str:
    return "\n".join(render(t) for t in ts)


def demo() -> List[Task]:
    ts: List[Task] = []
    ts = add_task(ts, "buy milk", Priority.LOW)
    ts = add_task(ts, "write report", Priority.HIGH)
    ts = add_task(ts, "call bob", Priority.MED)
    ts = add_task(ts, "fix sink", Priority.HIGH)
    return ts


if __name__ == "__main__":
    try:
        print(render_all(complete(demo(), 1)))
    except TaskErr as e:
        print(f"error: {e}")


def test_ids():
    assert [t.id for t in demo()] == [1, 2, 3, 4]


def test_complete_one():
    assert [t.id for t in pending(complete(demo(), 2))] == [1, 3, 4]


def test_render_one():
    assert render(Task(5, "nap", True, Priority.LOW)) == "[x] #5 (low) nap"


def test_complete_errors():
    import pytest
    with pytest.raises(NotFound) as e:
        complete(demo(), 9)
    assert e.value.id == 9
    with pytest.raises(AlreadyDone) as e:
        complete(complete(demo(), 1), 1)
    assert e.value.id == 1
    assert isinstance(e.value, TaskErr)


def test_render_label():
    assert render(demo()[1]) == "[ ] #2 (high) write report"


def test_by_priority():
    ts = complete(demo(), 2)
    assert [t.id for t in by_priority(ts)] == [4, 3, 1]


def test_clear_done():
    ts = complete(complete(demo(), 1), 3)
    assert [t.id for t in clear_done(ts)] == [2, 4]


def test_retitle():
    import pytest
    assert retitle(demo(), 3, "x")[2].title == "x"
    with pytest.raises(NotFound):
        retitle(demo(), 7, "x")
