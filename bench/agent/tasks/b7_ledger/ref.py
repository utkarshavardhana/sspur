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


class Unbalanced(LedgerErr):
    def __init__(self, id: int, diff: int):
        super().__init__(id, diff)
        self.id = id
        self.diff = diff


class TooFewPostings(LedgerErr):
    def __init__(self, id: int):
        super().__init__(id)
        self.id = id


class DuplicateId(LedgerErr):
    def __init__(self, id: int):
        super().__init__(id)
        self.id = id


class Overdrawn(LedgerErr):
    def __init__(self, account: str, balance: int):
        super().__init__(account, balance)
        self.account = account
        self.balance = balance


class UnknownTxn(LedgerErr):
    def __init__(self, id: int):
        super().__init__(id)
        self.id = id


def chart() -> List[str]:
    return ["income:salary", "assets:bank", "expenses:food", "assets:cash", "equity:owner"]


def new_ledger() -> Ledger:
    return Ledger(chart(), [])


def p(account: str, amount: int) -> Posting:
    return Posting(account, amount)


def add_txn(l: Ledger, t: Txn) -> Ledger:
    if len(t.postings) < 2:
        raise TooFewPostings(t.id)
    for x in t.postings:
        if x.account not in l.accounts:
            raise UnknownAccount(x.account)
    diff = sum(x.amount for x in t.postings)
    if diff != 0:
        raise Unbalanced(t.id, diff)
    if any(u.id == t.id for u in l.txns):
        raise DuplicateId(t.id)
    l2 = replace(l, txns=l.txns + [t])
    for x in t.postings:
        if x.account.startswith("assets:") and balance(l2, x.account) < 0:
            raise Overdrawn(x.account, balance(l2, x.account))
    return l2


def balance(l: Ledger, account: str) -> int:
    return sum(x.amount for t in l.txns for x in t.postings if x.account == account)


def trial_balance(l: Ledger) -> List[Tuple[str, int]]:
    return [(a, balance(l, a)) for a in sorted(l.accounts) if balance(l, a) != 0]


def reverse(l: Ledger, id: int, new_id: int) -> Ledger:
    for t in l.txns:
        if t.id == id:
            return add_txn(l, Txn(new_id, f"reverse {id}", [replace(x, amount=-x.amount) for x in t.postings]))
    raise UnknownTxn(id)


def history(l: Ledger, account: str) -> List[Tuple[int, int]]:
    bal = 0
    out = []
    for t in l.txns:
        ps = [x.amount for x in t.postings if x.account == account]
        if ps:
            bal += sum(ps)
            out.append((t.id, bal))
    return out


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


def test_unbalanced():
    try:
        add_txn(new_ledger(), Txn(1, "x", [p("assets:bank", 5), p("equity:owner", -4)]))
        assert False
    except Unbalanced as e:
        assert e.diff == 1


def test_reversed():
    assert history(reverse(sample(), 2, 3), "assets:bank") == [(1, 1000), (2, 970), (3, 1000)]
