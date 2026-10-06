from dataclasses import dataclass, replace
from typing import List, Tuple


@dataclass(frozen=True)
class Room:
    name: str
    exits: List[Tuple[str, str]]
    items: List[str]
    locked: List[str]


@dataclass(frozen=True)
class Game:
    room: str
    rooms: List[Room]
    inventory: List[str]
    moves: int


def world() -> List[Room]:
    return [
        Room("hall", [("north", "vault"), ("east", "kitchen")], [], ["north"]),
        Room("kitchen", [("west", "hall")], ["key", "bread"], []),
        Room("vault", [("south", "hall")], ["gold"], []),
    ]


def new_game() -> Game:
    return Game("hall", world(), [], 0)


def current(g: Game) -> Room:
    return next(r for r in g.rooms if r.name == g.room)


def look(g: Game) -> str:
    r = current(g)
    items = f" Items: {', '.join(sorted(r.items))}." if r.items else ""
    return f"You are in the {r.name}. Exits: " + ", ".join(sorted(d for d, _ in r.exits)) + "." + items


def go(g: Game, dir: str) -> Tuple[Game, str]:
    for d, target in current(g).exits:
        if d == dir:
            if dir in current(g).locked and "key" not in g.inventory:
                return g, "The door is locked."
            g2 = replace(g, room=target, moves=g.moves + 1)
            return g2, look(g2)
    return g, "You can't go that way."


def _set_items(g: Game, items: List[str]) -> Game:
    return replace(g, rooms=[replace(r, items=items) if r.name == g.room else r for r in g.rooms])


def take(g: Game, item: str) -> Tuple[Game, str]:
    if item in current(g).items:
        g2 = _set_items(g, [i for i in current(g).items if i != item])
        return replace(g2, inventory=g.inventory + [item]), "Taken."
    return g, f"There is no {item} here."


def drop(g: Game, item: str) -> Tuple[Game, str]:
    if item in g.inventory:
        g2 = _set_items(g, current(g).items + [item])
        return replace(g2, inventory=[i for i in g.inventory if i != item]), "Dropped."
    return g, f"You don't have {item}."


def step(g: Game, cmd: str) -> Tuple[Game, str]:
    w = cmd.lower().split()
    if w == ["look"]:
        return g, look(g)
    if w == ["inventory"]:
        return g, ("You carry: " + ", ".join(sorted(g.inventory)) + ".") if g.inventory else "You carry nothing."
    if len(w) == 2 and w[0] == "go":
        return go(g, w[1])
    if len(w) == 2 and w[0] == "take":
        return take(g, w[1])
    if len(w) == 2 and w[0] == "drop":
        return drop(g, w[1])
    return g, "I don't understand."


def run(cmds: List[str]) -> Tuple[Game, List[str]]:
    g = new_game()
    out = []
    for c in cmds:
        g, msg = step(g, c)
        out.append(msg)
    return g, out


def play(cmds: List[str]) -> List[str]:
    return run(cmds)[1]


def score(g: Game) -> int:
    return 10 * len(g.inventory) + (25 if g.room == "vault" else 0)


if __name__ == "__main__":
    for line in play(["look", "go east", "go west"]):
        print(line)


def test_starts_in_hall():
    assert play(["look"]) == ["You are in the hall. Exits: east, north."]


def test_walks():
    assert play(["go east", "go west"])[-1] == "You are in the hall. Exits: east, north."


def test_blocked():
    assert play(["go up"]) == ["You can't go that way."]


def test_counts_moves():
    assert run(["go east", "go west"])[0].moves == 2


def test_locked():
    assert play(["go north"]) == ["The door is locked."]


def test_takes():
    assert play(["go east", "take key", "inventory"])[-1] == "You carry: key."


def test_scored():
    assert score(run(["go east", "take key"])[0]) == 10
