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


def install_order(reg: List[Pkg], target: str) -> List[str]:
    p = find_pkg(reg, target)
    out: List[str] = []
    for d in p.deps:
        out += install_order(reg, d)
    return out + [target]


def plan(reg: List[Pkg], target: str) -> str:
    try:
        return " -> ".join(install_order(reg, target))
    except Unknown as e:
        return f"error: unknown package {e.name}"


if __name__ == "__main__":
    print(plan(registry(), "app"))


def test_leaf():
    assert install_order(registry(), "http") == ["http"]


def test_db_first_log():
    assert install_order(registry(), "db") == ["log", "db"]


def test_unknown():
    assert plan(registry(), "nope") == "error: unknown package nope"
