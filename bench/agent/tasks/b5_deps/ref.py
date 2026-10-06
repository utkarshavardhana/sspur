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


def registry() -> List[Pkg]:
    return [Pkg("app", ["web", "db"]), Pkg("web", ["http", "log"]), Pkg("db", ["log"]), Pkg("http", []), Pkg("log", [])]


def find_pkg(reg: List[Pkg], name: str) -> Pkg:
    for p in reg:
        if p.name == name:
            return p
    raise Unknown(name)


def _visit(reg: List[Pkg], name: str, chain: List[str], done: List[str]) -> None:
    if name in done:
        return
    if name in chain:
        raise Cycle(chain[chain.index(name):] + [name])
    p = find_pkg(reg, name)
    for d in p.deps:
        if not any(q.name == d for q in reg):
            raise Missing(name, d)
        _visit(reg, d, chain + [name], done)
    done.append(name)


def install_order(reg: List[Pkg], target: str) -> List[str]:
    done: List[str] = []
    _visit(reg, target, [], done)
    return done


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
    done: List[str] = []
    while True:
        ready = sorted(p.name for p in reg if p.name not in done and all(d in done for d in p.deps))
        if not ready:
            break
        done.append(ready[0])
    if len(done) < len(reg):
        raise Stuck(sorted(p.name for p in reg if p.name not in done))
    return done


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
    return sorted(found - {name})


if __name__ == "__main__":
    print(plan(registry(), "app"))


def test_leaf():
    assert install_order(registry(), "http") == ["http"]


def test_db_first_log():
    assert install_order(registry(), "db") == ["log", "db"]


def test_unknown():
    assert plan(registry(), "nope") == "error: unknown package nope"


def test_once():
    assert plan(registry(), "app") == "http -> log -> web -> db -> app"


def test_cycle():
    assert plan([Pkg("a", ["a"])], "a") == "error: cycle a -> a"


def test_full():
    assert full_order(registry()) == ["http", "log", "db", "web", "app"]


def test_deps_of_log():
    assert dependents(registry(), "log") == ["app", "db", "web"]
