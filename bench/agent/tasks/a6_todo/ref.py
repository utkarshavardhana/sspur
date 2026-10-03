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


class TaskErr(Exception):
    pass


class NotFound(TaskErr):
    def __init__(self, id: int):
        super().__init__(id)
        self.id = id


class AlreadyDone(TaskErr):
    def __init__(self, id: int):
        super().__init__(id)
        self.id = id


def find_task(ts: List[Task], id: int) -> Task:
    for t in ts:
        if t.id == id:
            return t
    raise NotFound(id)


def next_id(ts: List[Task]) -> int:
    return max((t.id for t in ts), default=0) + 1


def add_task(ts: List[Task], title: str, pri: Priority) -> List[Task]:
    return ts + [Task(next_id(ts), title, False, pri)]


def complete(ts: List[Task], id: int) -> List[Task]:
    if find_task(ts, id).done:
        raise AlreadyDone(id)
    return [replace(t, done=True) if t.id == id else t for t in ts]


def retitle(ts: List[Task], id: int, title: str) -> List[Task]:
    find_task(ts, id)
    return [replace(t, title=title) if t.id == id else t for t in ts]


def clear_done(ts: List[Task]) -> List[Task]:
    return [t for t in ts if not t.done]


RANK = {Priority.HIGH: 0, Priority.MED: 1, Priority.LOW: 2}


def by_priority(ts: List[Task]) -> List[Task]:
    return sorted(pending(ts), key=lambda t: (RANK[t.pri], t.id))


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
    print(render_all(complete(demo(), 1)))


def test_ids():
    assert [t.id for t in demo()] == [1, 2, 3, 4]


def test_complete_one():
    assert [t.id for t in pending(complete(demo(), 2))] == [1, 3, 4]


def test_render_one():
    assert render(Task(5, "nap", True, Priority.LOW)) == "[x] #5 (low) nap"
