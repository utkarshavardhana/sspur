from dataclasses import dataclass, replace
from typing import List, Tuple


@dataclass(frozen=True)
class Posting:
    account: str
    amount: int


@dataclass(frozen=True)
class Txn:
    id: int
    memo: str
    postings: List[Posting]


@dataclass(frozen=True)
class Ledger:
    accounts: List[str]
    txns: List[Txn]


class LedgerErr(Exception):
    pass


class UnknownAccount(LedgerErr):
    def __init__(self, name: str):
        super().__init__(name)
        self.name = name


def chart() -> List[str]:
    return ["income:salary", "assets:bank", "expenses:food", "assets:cash", "equity:owner"]


def new_ledger() -> Ledger:
    return Ledger(chart(), [])


def p(account: str, amount: int) -> Posting:
    return Posting(account, amount)


def add_txn(l: Ledger, t: Txn) -> Ledger:
    for x in t.postings:
        if x.account not in l.accounts:
            raise UnknownAccount(x.account)
    return replace(l, txns=l.txns + [t])


def balance(l: Ledger, account: str) -> int:
    return sum(x.amount for t in l.txns for x in t.postings if x.account == account)


def trial_balance(l: Ledger) -> List[Tuple[str, int]]:
    return [(a, balance(l, a)) for a in l.accounts]


def sample() -> Ledger:
    l = add_txn(new_ledger(), Txn(1, "open", [p("assets:bank", 1000), p("equity:owner", -1000)]))
    return add_txn(l, Txn(2, "lunch", [p("expenses:food", 30), p("assets:bank", -30)]))


if __name__ == "__main__":
    print("\n".join(f"{a} {b}" for a, b in trial_balance(sample())))


def test_bank():
    assert balance(sample(), "assets:bank") == 970


def test_unknown():
    try:
        add_txn(new_ledger(), Txn(1, "x", [p("assets:gold", 5), p("equity:owner", -5)]))
        assert False
    except UnknownAccount as e:
        assert e.name == "assets:gold"


def test_trial_sums_to_zero():
    assert sum(b for _, b in trial_balance(sample())) == 0
