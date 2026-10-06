import pytest
from app import *

HALL = "You are in the hall. Exits: east, north."
KITCHEN = "You are in the kitchen. Exits: west. Items: bread, key."


def test_hidden_look_sorted():
    assert play(["look"]) == [HALL]


def test_hidden_items_shown():
    assert play(["go east"]) == [KITCHEN]


def test_hidden_case_spaces():
    assert play(["  GO   East ", "LOOK", "Look  "]) == [KITCHEN, KITCHEN, KITCHEN]


def test_hidden_unknown():
    assert play(["dance", "", "go", "go east west"]) == ["I don't understand."] * 4


def test_hidden_no_exit():
    assert play(["go west"]) == ["You can't go that way."]


def test_hidden_take():
    assert play(["go east", "take key", "look"]) == [KITCHEN, "Taken.", "You are in the kitchen. Exits: west. Items: bread."]


def test_hidden_take_case():
    assert play(["go east", "TAKE Key", "inventory"])[1:] == ["Taken.", "You carry: key."]


def test_hidden_take_missing():
    assert play(["take key"]) == ["There is no key here."]


def test_hidden_inventory_empty():
    assert play(["inventory"]) == ["You carry nothing."]


def test_hidden_inventory_sorted():
    assert play(["go east", "take key", "take bread", "inventory"])[-1] == "You carry: bread, key."


def test_hidden_drop():
    assert play(["go east", "take key", "go west", "drop key", "look", "inventory"])[2:] == [HALL, "Dropped.", "You are in the hall. Exits: east, north. Items: key.", "You carry nothing."]


def test_hidden_drop_missing():
    assert play(["drop gold"]) == ["You don't have gold."]


def test_hidden_locked():
    assert play(["go north"]) == ["The door is locked."] and run(["go north"])[0].room == "hall"


def test_hidden_unlocked():
    assert play(["go east", "take key", "go west", "go north"])[-1] == "You are in the vault. Exits: south. Items: gold."


def test_hidden_moves():
    assert run(["go east", "go up", "go west", "go north"])[0].moves == 2


def test_hidden_world_locked():
    assert [list(r.locked) for r in world()] == [["north"], [], []]


def test_hidden_score():
    assert score(new_game()) == 0
    assert score(run(["go east", "take key", "take bread"])[0]) == 20
    assert score(run(["go east", "take key", "go west", "go north", "take gold"])[0]) == 45
