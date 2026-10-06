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
    s = f"You are in the {r.name}. Exits: " + ", ".join(sorted(d for d, _ in r.exits))
    if r.items:
        s += ". Items: " + ", ".join(sorted(r.items))
    return s + "."


def go(g: Game, dir: str) -> Tuple[Game, str]:
    r = current(g)
    for d, target in r.exits:
        if d == dir:
            if d in r.locked and "key" not in g.inventory:
                return g, "The door is locked."
            g2 = replace(g, room=target, moves=g.moves + 1)
            return g2, look(g2)
    return g, "You can't go that way."


def _set_room(g: Game, r: Room) -> List[Room]:
    return [r if x.name == r.name else x for x in g.rooms]


def take(g: Game, item: str) -> Tuple[Game, str]:
    r = current(g)
    if item not in r.items:
        return g, f"There is no {item} here."
    items = list(r.items)
    items.remove(item)
    r2 = replace(r, items=items)
    return replace(g, rooms=_set_room(g, r2), inventory=g.inventory + [item]), "Taken."


def drop(g: Game, item: str) -> Tuple[Game, str]:
    if item not in g.inventory:
        return g, f"You don't have {item}."
    inv = list(g.inventory)
    inv.remove(item)
    r = current(g)
    r2 = replace(r, items=r.items + [item])
    return replace(g, rooms=_set_room(g, r2), inventory=inv), "Dropped."


def inventory(g: Game) -> str:
    if not g.inventory:
        return "You carry nothing."
    return "You carry: " + ", ".join(sorted(g.inventory)) + "."


def score(g: Game) -> int:
    return 10 * len(g.inventory) + (25 if g.room == "vault" else 0)


def step(g: Game, cmd: str) -> Tuple[Game, str]:
    w = cmd.lower().split()
    if w == ["look"]:
        return g, look(g)
    if w == ["inventory"]:
        return g, inventory(g)
    if len(w) == 2:
        if w[0] == "go":
            return go(g, w[1])
        if w[0] == "take":
            return take(g, w[1])
        if w[0] == "drop":
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


def test_parser_forgiving():
    assert play(["  GO   East "]) == ["You are in the kitchen. Exits: west. Items: bread, key."]
    assert play(["LOOK"]) == play(["look"])
    for c in ["", "   ", "dance", "go", "go east now", "look around", "take", "inventory x"]:
        assert play([c]) == ["I don't understand."]


def test_look_items_sorted():
    assert play(["go east"])[0] == "You are in the kitchen. Exits: west. Items: bread, key."


def test_take_drop_inventory():
    g, out = run(["inventory", "go east", "take key", "take key", "take bread", "inventory",
                  "go west", "drop key", "drop key", "look", "inventory"])
    assert out[0] == "You carry nothing."
    assert out[2] == "Taken."
    assert out[3] == "There is no key here."
    assert out[5] == "You carry: bread, key."
    assert out[7] == "Dropped."
    assert out[8] == "You don't have key."
    assert out[9] == "You are in the hall. Exits: east, north. Items: key."
    assert out[10] == "You carry: bread."
    assert g.inventory == ["bread"]


def test_locked_door():
    g, out = run(["go north"])
    assert out == ["The door is locked."]
    assert g.room == "hall" and g.moves == 0
    g, out = run(["go east", "take key", "go west", "go north"])
    assert out[-1] == "You are in the vault. Exits: south. Items: gold."
    assert g.moves == 3
    assert [r.locked for r in world()] == [["north"], [], []]


def test_score():
    assert score(new_game()) == 0
    g, _ = run(["go east", "take key", "take bread"])
    assert score(g) == 20
    g, _ = run(["go east", "take key", "go west", "go north"])
    assert score(g) == 35
