import pytest
from app import *


def h_order(reg, target):
    try:
        return ",".join(install_order(reg, target))
    except Unknown as e:
        return f"unknown {e.name}"
    except Missing as e:
        return f"missing {e.pkg} {e.dep}"
    except Cycle as e:
        return "cycle " + ",".join(e.path)
    except DepErr:
        return "other"


def h_full(reg):
    try:
        return ",".join(full_order(reg))
    except Stuck as e:
        return "stuck " + ",".join(e.names)
    except DepErr:
        return "other"


def h_deps(reg, name):
    try:
        return ",".join(dependents(reg, name))
    except Unknown as e:
        return f"unknown {e.name}"
    except DepErr:
        return "other"


def h_diamond():
    return [Pkg("a", ["b", "c"]), Pkg("b", ["d"]), Pkg("c", ["d"]), Pkg("d", [])]


def test_hidden_once():
    assert h_order(registry(), "app") == "http,log,web,db,app"


def test_hidden_leaf():
    assert h_order(registry(), "log") == "log"


def test_hidden_diamond():
    assert h_order(h_diamond(), "a") == "d,b,c,a"


def test_hidden_unknown():
    assert h_order(registry(), "zzz") == "unknown zzz"


def test_hidden_missing():
    assert h_order([Pkg("a", ["b"]), Pkg("b", ["x"])], "a") == "missing b x"


def test_hidden_cycle():
    assert h_order([Pkg("a", ["b"]), Pkg("b", ["c"]), Pkg("c", ["a"])], "a") == "cycle a,b,c,a"


def test_hidden_self_cycle():
    assert h_order([Pkg("a", ["a"])], "a") == "cycle a,a"


def test_hidden_deep_cycle():
    assert h_order([Pkg("top", ["b"]), Pkg("b", ["c"]), Pkg("c", ["b"])], "top") == "cycle b,c,b"


def test_hidden_full():
    assert h_full(registry()) == "http,log,db,web,app"


def test_hidden_full_diamond():
    assert h_full(h_diamond()) == "d,b,c,a"


def test_hidden_full_stuck():
    assert h_full([Pkg("a", ["b"]), Pkg("b", ["a"]), Pkg("c", [])]) == "stuck a,b"


def test_hidden_full_missing():
    assert h_full([Pkg("a", ["x"]), Pkg("b", [])]) == "stuck a"


def test_hidden_full_empty():
    assert h_full([]) == ""


def test_hidden_dependents():
    assert h_deps(registry(), "log") == "app,db,web" and h_deps(registry(), "http") == "app,web"


def test_hidden_dependents_none():
    assert h_deps(registry(), "app") == "" and h_deps(h_diamond(), "d") == "a,b,c"


def test_hidden_dependents_unknown():
    assert h_deps(registry(), "zzz") == "unknown zzz"


def test_hidden_plan():
    assert plan(registry(), "app") == "http -> log -> web -> db -> app"


def test_hidden_plan_errors():
    assert plan([Pkg("a", ["b"]), Pkg("b", ["x"])], "a") == "error: b needs missing x"
    assert plan([Pkg("a", ["b"]), Pkg("b", ["a"])], "a") == "error: cycle a -> b -> a"
