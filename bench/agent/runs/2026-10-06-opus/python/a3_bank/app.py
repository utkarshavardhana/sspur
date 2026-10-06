from dataclasses import dataclass, replace
from typing import List


@dataclass(frozen=True)
class Account:
    id: str
    owner: str
    balance: int
    frozen: bool = False


class BankErr(Exception):
    pass


class Insufficient(BankErr):
    def __init__(self, id: str, needed: int, available: int):
        super().__init__(id, needed, available)
        self.id = id
        self.needed = needed
        self.available = available


class SameAccount(BankErr):
    pass


class UnknownAccount(BankErr):
    def __init__(self, id: str):
        super().__init__(id)
        self.id = id


class Frozen(BankErr):
    def __init__(self, id: str):
        super().__init__(id)
        self.id = id


def accounts() -> List[Account]:
    return [
        Account("a1", "alice", 100, False),
        Account("b1", "bob", 20, False),
        Account("c1", "carol", 0, False),
    ]


def open_account(bank: List[Account], id: str, owner: str) -> List[Account]:
    return bank + [Account(id, owner, 0, False)]


def lookup(bank: List[Account], id: str) -> Account:
    for a in bank:
        if a.id == id:
            return a
    raise UnknownAccount(id)


def freeze(bank: List[Account], id: str) -> List[Account]:
    lookup(bank, id)
    return [replace(a, frozen=True) if a.id == id else a for a in bank]


def set_balance(bank: List[Account], id: str, b: int) -> List[Account]:
    assert b >= 0
    return [replace(a, balance=b) if a.id == id else a for a in bank]


def deposit(bank: List[Account], id: str, amount: int) -> List[Account]:
    assert amount > 0
    a = lookup(bank, id)
    if a.frozen:
        raise Frozen(id)
    return set_balance(bank, id, a.balance + amount)


def withdraw(bank: List[Account], id: str, amount: int) -> List[Account]:
    assert amount > 0
    a = lookup(bank, id)
    if a.frozen:
        raise Frozen(id)
    if amount > a.balance:
        raise Insufficient(id, amount, a.balance)
    return set_balance(bank, id, a.balance - amount)


def transfer(bank: List[Account], frm: str, to: str, amount: int) -> List[Account]:
    if frm == to:
        raise SameAccount()
    fee = 1 if amount < 50 else 0
    return deposit(withdraw(bank, frm, amount + fee), to, amount)


def describe(a: Account) -> str:
    return f"{a.owner}: {a.balance}"


def attempt(bank: List[Account], frm: str, to: str, amount: int) -> str:
    try:
        return ", ".join(describe(a) for a in transfer(bank, frm, to, amount))
    except Insufficient as e:
        return f"declined: {e.id} needs {e.needed}, has {e.available}"
    except SameAccount:
        return "declined: same account"
    except UnknownAccount as e:
        return f"declined: no account {e.id}"
    except Frozen as e:
        return f"declined: {e.id} is frozen"


if __name__ == "__main__":
    print(attempt(accounts(), "a1", "b1", 60))
    print(attempt(accounts(), "b1", "a1", 50))


def test_big_transfer():
    assert attempt(accounts(), "a1", "b1", 60) == "alice: 40, bob: 80, carol: 0"


def test_declined():
    assert attempt(accounts(), "b1", "a1", 50) == "declined: b1 needs 50, has 20"


def test_unknown():
    assert attempt(accounts(), "a1", "zz", 5) == "declined: no account zz"


def test_opened():
    assert describe(open_account(accounts(), "d1", "dan")[-1]) == "dan: 0"



def test_starts_unfrozen():
    assert all(not a.frozen for a in accounts())
    assert open_account(accounts(), "d1", "dan")[-1] == Account("d1", "dan", 0, False)


def test_freeze():
    bank = freeze(accounts(), "b1")
    assert lookup(bank, "b1").frozen
    assert not lookup(bank, "a1").frozen


def test_freeze_unknown():
    try:
        freeze(accounts(), "zz")
        assert False
    except UnknownAccount as e:
        assert e.id == "zz"


def test_frozen_deposit_withdraw():
    bank = freeze(accounts(), "c1")
    for op in (deposit, withdraw):
        try:
            op(bank, "c1", 5)
            assert False
        except Frozen as e:
            assert e.id == "c1"


def test_frozen_attempt():
    assert attempt(freeze(accounts(), "a1"), "a1", "b1", 60) == "declined: a1 is frozen"
    assert attempt(freeze(accounts(), "b1"), "a1", "b1", 60) == "declined: b1 is frozen"


def test_fee():
    assert attempt(accounts(), "a1", "b1", 10) == "alice: 89, bob: 30, carol: 0"
    assert attempt(accounts(), "a1", "b1", 50) == "alice: 50, bob: 70, carol: 0"


def test_fee_insufficient():
    assert attempt(accounts(), "b1", "a1", 20) == "declined: b1 needs 21, has 20"
    assert attempt(accounts(), "b1", "a1", 19) == "alice: 119, bob: 0, carol: 0"
