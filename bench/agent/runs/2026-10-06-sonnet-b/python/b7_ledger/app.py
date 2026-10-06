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


class TooFewPostings(LedgerErr):
    def __init__(self, id: int):
        super().__init__(id)
        self.id = id


class Unbalanced(LedgerErr):
    def __init__(self, id: int, diff: int):
        super().__init__(id, diff)
        self.id = id
        self.diff = diff


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


def balance(l: Ledger, account: str) -> int:
    return sum(x.amount for t in l.txns for x in t.postings if x.account == account)


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
    for x in t.postings:
        if x.account.startswith("assets:"):
            b = balance(l, x.account) + sum(y.amount for y in t.postings if y.account == x.account)
            if b < 0:
                raise Overdrawn(x.account, b)
    return replace(l, txns=l.txns + [t])


def trial_balance(l: Ledger) -> List[Tuple[str, int]]:
    return [(a, b) for a in sorted(l.accounts) for b in [balance(l, a)] if b != 0]


def reverse(l: Ledger, id: int, new_id: int) -> Ledger:
    for t in l.txns:
        if t.id == id:
            return add_txn(l, Txn(new_id, "reverse %s" % id, [p(x.account, -x.amount) for x in t.postings]))
    raise UnknownTxn(id)


def history(l: Ledger, account: str) -> List[Tuple[int, int]]:
    out = []
    bal = 0
    for t in l.txns:
        touched = [x for x in t.postings if x.account == account]
        if touched:
            bal += sum(x.amount for x in touched)
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


def _raises(exc, f):
    try:
        f()
    except exc as e:
        return e
    assert False


def test_too_few():
    e = _raises(TooFewPostings, lambda: add_txn(new_ledger(), Txn(7, "x", [p("assets:bank", 0)])))
    assert e.id == 7


def test_order_unknown_before_unbalanced():
    _raises(UnknownAccount, lambda: add_txn(new_ledger(), Txn(1, "x", [p("nope", 5), p("equity:owner", -4)])))


def test_unbalanced():
    e = _raises(Unbalanced, lambda: add_txn(new_ledger(), Txn(1, "x", [p("assets:bank", 5), p("equity:owner", -3)])))
    assert (e.id, e.diff) == (1, 2)


def test_duplicate():
    e = _raises(DuplicateId, lambda: add_txn(sample(), Txn(1, "x", [p("assets:bank", 5), p("equity:owner", -5)])))
    assert e.id == 1


def test_duplicate_before_overdrawn():
    _raises(DuplicateId, lambda: add_txn(sample(), Txn(2, "x", [p("assets:bank", -5000), p("expenses:food", 5000)])))


def test_overdrawn():
    e = _raises(Overdrawn, lambda: add_txn(sample(), Txn(3, "x", [p("expenses:food", 1000), p("assets:bank", -1000)])))
    assert (e.account, e.balance) == ("assets:bank", -30)


def test_overdrawn_first_in_order():
    e = _raises(Overdrawn, lambda: add_txn(new_ledger(), Txn(1, "x", [p("assets:cash", -1), p("assets:bank", -2), p("equity:owner", 3)])))
    assert (e.account, e.balance) == ("assets:cash", -1)


def test_non_asset_may_go_negative():
    assert balance(sample(), "equity:owner") == -1000


def test_trial_balance_filtered_sorted():
    assert trial_balance(sample()) == [("assets:bank", 970), ("equity:owner", -1000), ("expenses:food", 30)]


def test_reverse():
    l = reverse(sample(), 2, 3)
    t = l.txns[-1]
    assert t.id == 3 and t.memo == "reverse 2"
    assert t.postings == [p("expenses:food", -30), p("assets:bank", 30)]
    assert balance(l, "assets:bank") == 1000


def test_reverse_unknown():
    e = _raises(UnknownTxn, lambda: reverse(sample(), 9, 10))
    assert e.id == 9


def test_reverse_rules_apply():
    _raises(DuplicateId, lambda: reverse(sample(), 2, 1))
    _raises(Overdrawn, lambda: reverse(sample(), 1, 5))


def test_history():
    l = reverse(sample(), 2, 3)
    assert history(l, "assets:bank") == [(1, 1000), (2, 970), (3, 1000)]
    assert history(l, "assets:cash") == []
