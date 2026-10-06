from dataclasses import dataclass, replace
from enum import Enum
from typing import List


class Priority(Enum):
    LOW = "low"
    MED = "med"
    HIGH = "high"


class TaskErr(Exception):
    pass


class NotFound(TaskErr):
    def __init__(self, id: int):
        self.id = id
        super().__init__(f"Task {id} not found")


class AlreadyDone(TaskErr):
    def __init__(self, id: int):
        self.id = id
        super().__init__(f"Task {id} is already done")


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


def complete(ts: List[Task], id: int) -> List[Task]:
    for t in ts:
        if t.id == id:
            if t.done:
                raise AlreadyDone(id)
            return [replace(t, done=True) if t.id == id else t for t in ts]
    raise NotFound(id)


def pending(ts: List[Task]) -> List[Task]:
    return [t for t in ts if not t.done]


def box(t: Task) -> str:
    return "[x]" if t.done else "[ ]"


def render(t: Task) -> str:
    return f"{box(t)} #{t.id} ({t.pri.value}) {t.title}"


def render_all(ts: List[Task]) -> str:
    return "\n".join(render(t) for t in ts)


def by_priority(ts: List[Task]) -> List[Task]:
    pending_tasks = pending(ts)
    priority_order = {Priority.HIGH: 0, Priority.MED: 1, Priority.LOW: 2}
    return sorted(pending_tasks, key=lambda t: (priority_order[t.pri], t.id))


def clear_done(ts: List[Task]) -> List[Task]:
    return [t for t in ts if not t.done]


def retitle(ts: List[Task], id: int, title: str) -> List[Task]:
    for t in ts:
        if t.id == id:
            return [replace(t, title=title) if t.id == id else t for t in ts]
    raise NotFound(id)


def demo() -> List[Task]:
    ts: List[Task] = []
    ts = add_task(ts, "buy milk", Priority.LOW)
    ts = add_task(ts, "write report", Priority.HIGH)
    ts = add_task(ts, "call bob", Priority.MED)
    ts = add_task(ts, "fix sink", Priority.HIGH)
    return ts


if __name__ == "__main__":
    ts = demo()
    try:
        ts = complete(ts, 1)
    except TaskErr as e:
        print(f"Error: {e}")
    print(render_all(ts))


def test_ids():
    assert [t.id for t in demo()] == [1, 2, 3, 4]


def test_complete_one():
    ts = demo()
    ts = complete(ts, 2)
    assert [t.id for t in pending(ts)] == [1, 3, 4]


def test_render_one():
    assert render(Task(5, "nap", True, Priority.LOW)) == "[x] #5 (low) nap"


def test_complete_not_found():
    import pytest
    ts = demo()
    with pytest.raises(NotFound) as exc_info:
        complete(ts, 99)
    assert exc_info.value.id == 99


def test_complete_already_done():
    import pytest
    ts = demo()
    ts = complete(ts, 1)
    with pytest.raises(AlreadyDone) as exc_info:
        complete(ts, 1)
    assert exc_info.value.id == 1


def test_by_priority():
    ts = demo()
    result = by_priority(ts)
    assert [t.id for t in result] == [2, 4, 3, 1]


def test_clear_done():
    ts = demo()
    ts = complete(ts, 1)
    ts = complete(ts, 3)
    result = clear_done(ts)
    assert [t.id for t in result] == [2, 4]


def test_retitle():
    ts = demo()
    ts = retitle(ts, 2, "new title")
    task = next(t for t in ts if t.id == 2)
    assert task.title == "new title"


def test_retitle_not_found():
    import pytest
    ts = demo()
    with pytest.raises(NotFound) as exc_info:
        retitle(ts, 99, "new title")
    assert exc_info.value.id == 99
