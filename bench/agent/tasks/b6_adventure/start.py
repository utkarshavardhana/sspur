from dataclasses import dataclass, replace
from typing import List, Tuple


@dataclass(frozen=True)
class Room:
    name: str
    exits: List[Tuple[str, str]]
    items: List[str]


@dataclass(frozen=True)
class Game:
    room: str
    rooms: List[Room]
    inventory: List[str]
    moves: int


def world() -> List[Room]:
    return [
        Room("hall", [("north", "vault"), ("east", "kitchen")], []),
        Room("kitchen", [("west", "hall")], ["key", "bread"]),
        Room("vault", [("south", "hall")], ["gold"]),
    ]


def new_game() -> Game:
    return Game("hall", world(), [], 0)


def current(g: Game) -> Room:
    return next(r for r in g.rooms if r.name == g.room)


def look(g: Game) -> str:
    r = current(g)
    return f"You are in the {r.name}. Exits: " + ", ".join(d for d, _ in r.exits) + "."


def go(g: Game, dir: str) -> Tuple[Game, str]:
    for d, target in current(g).exits:
        if d == dir:
            g2 = replace(g, room=target, moves=g.moves + 1)
            return g2, look(g2)
    return g, "You can't go that way."


def step(g: Game, cmd: str) -> Tuple[Game, str]:
    w = cmd.split(" ")
    if w[0] == "look":
        return g, look(g)
    if w[0] == "go" and len(w) == 2:
        return go(g, w[1])
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
    assert play(["look"]) == ["You are in the hall. Exits: north, east."]


def test_walks():
    assert play(["go east", "go west"])[-1] == "You are in the hall. Exits: north, east."


def test_blocked():
    assert play(["go up"]) == ["You can't go that way."]


def test_counts_moves():
    assert run(["go east", "go west"])[0].moves == 2
