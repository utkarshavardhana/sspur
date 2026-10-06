from dataclasses import dataclass
from typing import List


@dataclass(frozen=True)
class Pkg:
    name: str
    deps: List[str]


class DepErr(Exception):
    pass


class Unknown(DepErr):
    def __init__(self, name: str):
        super().__init__(name)
        self.name = name


def registry() -> List[Pkg]:
    return [Pkg("app", ["web", "db"]), Pkg("web", ["http", "log"]), Pkg("db", ["log"]), Pkg("http", []), Pkg("log", [])]


def find_pkg(reg: List[Pkg], name: str) -> Pkg:
    for p in reg:
        if p.name == name:
            return p
    raise Unknown(name)


class Missing(DepErr):
    def __init__(self, pkg: str, dep: str):
        super().__init__(pkg, dep)
        self.pkg = pkg
        self.dep = dep


class Cycle(DepErr):
    def __init__(self, path: List[str]):
        super().__init__(path)
        self.path = path


class Stuck(DepErr):
    def __init__(self, names: List[str]):
        super().__init__(names)
        self.names = names


def install_order(reg: List[Pkg], target: str) -> List[str]:
    find_pkg(reg, target)
    by_name = {p.name: p for p in reg}
    out: List[str] = []
    done = set()
    chain: List[str] = []

    def visit(name: str) -> None:
        if name in chain:
            raise Cycle(chain[chain.index(name):] + [name])
        if name in done:
            return
        chain.append(name)
        for d in by_name[name].deps:
            if d not in by_name:
                raise Missing(name, d)
            visit(d)
        chain.pop()
        done.add(name)
        out.append(name)

    visit(target)
    return out


def plan(reg: List[Pkg], target: str) -> str:
    try:
        return " -> ".join(install_order(reg, target))
    except Unknown as e:
        return f"error: unknown package {e.name}"
    except Missing as e:
        return f"error: {e.pkg} needs missing {e.dep}"
    except Cycle as e:
        return "error: cycle " + " -> ".join(e.path)


def full_order(reg: List[Pkg]) -> List[str]:
    remaining = {p.name: p for p in reg}
    installed = set()
    out: List[str] = []
    while remaining:
        ready = sorted(n for n, p in remaining.items() if all(d in installed for d in p.deps))
        if not ready:
            raise Stuck(sorted(remaining))
        n = ready[0]
        del remaining[n]
        installed.add(n)
        out.append(n)
    return out


def dependents(reg: List[Pkg], name: str) -> List[str]:
    find_pkg(reg, name)
    found = {name}
    changed = True
    while changed:
        changed = False
        for p in reg:
            if p.name not in found and any(d in found for d in p.deps):
                found.add(p.name)
                changed = True
    found.discard(name)
    return sorted(found)


if __name__ == "__main__":
    print(plan(registry(), "app"))


def test_leaf():
    assert install_order(registry(), "http") == ["http"]


def test_db_first_log():
    assert install_order(registry(), "db") == ["log", "db"]


def test_unknown():
    assert plan(registry(), "nope") == "error: unknown package nope"


def test_shared_dep_once():
    assert install_order(registry(), "app") == ["http", "log", "web", "db", "app"]


def test_missing():
    reg = [Pkg("a", ["b"]), Pkg("b", ["zz"])]
    try:
        install_order(reg, "a")
        assert False
    except Missing as e:
        assert (e.pkg, e.dep) == ("b", "zz")
    assert plan(reg, "a") == "error: b needs missing zz"


def test_cycle():
    reg = [Pkg("x", ["a"]), Pkg("a", ["b"]), Pkg("b", ["c"]), Pkg("c", ["a"])]
    try:
        install_order(reg, "x")
        assert False
    except Cycle as e:
        assert e.path == ["a", "b", "c", "a"]
    assert plan(reg, "x") == "error: cycle a -> b -> c -> a"
    try:
        install_order([Pkg("a", ["a"])], "a")
        assert False
    except Cycle as e:
        assert e.path == ["a", "a"]


def test_full_order():
    assert full_order(registry()) == ["http", "log", "db", "web", "app"]
    try:
        full_order([Pkg("a", ["b"]), Pkg("b", ["a"]), Pkg("c", [])])
        assert False
    except Stuck as e:
        assert e.names == ["a", "b"]


def test_dependents():
    assert dependents(registry(), "log") == ["app", "db", "web"]
    assert dependents(registry(), "app") == []
    try:
        dependents(registry(), "nope")
        assert False
    except Unknown as e:
        assert e.name == "nope"
