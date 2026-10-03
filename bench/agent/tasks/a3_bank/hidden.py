import pytest
from app import *


def test_hidden_no_fee():
    assert attempt(accounts(), "a1", "b1", 60) == "alice: 40, bob: 80, carol: 0"


def test_hidden_fee():
    assert attempt(accounts(), "a1", "b1", 10) == "alice: 89, bob: 30, carol: 0"


def test_hidden_fee_short():
    assert attempt(accounts(), "b1", "a1", 20) == "declined: b1 needs 21, has 20"


def test_hidden_fee_exact():
    assert attempt(accounts(), "b1", "a1", 19) == "alice: 119, bob: 0, carol: 0"


def test_hidden_frozen_to():
    assert attempt(freeze(accounts(), "b1"), "a1", "b1", 60) == "declined: b1 is frozen"


def test_hidden_frozen_from():
    assert attempt(freeze(accounts(), "a1"), "a1", "b1", 60) == "declined: a1 is frozen"


def test_hidden_frozen_other():
    assert attempt(freeze(accounts(), "c1"), "a1", "b1", 60) == "alice: 40, bob: 80, carol: 0"


def test_hidden_deposit_frozen():
    with pytest.raises(Frozen) as e:
        deposit(freeze(accounts(), "c1"), "c1", 5)
    assert e.value.id == "c1"
    assert isinstance(e.value, BankErr)


def test_hidden_withdraw_frozen():
    with pytest.raises(Frozen) as e:
        withdraw([Account("z", "zed", 5, True)], "z", 1)
    assert e.value.id == "z"


def test_hidden_freeze_unknown():
    with pytest.raises(UnknownAccount) as e:
        freeze(accounts(), "q")
    assert e.value.id == "q"


def test_hidden_freeze_sets():
    assert [a.frozen for a in freeze(accounts(), "b1")] == [False, True, False]


def test_hidden_unfrozen():
    assert all(not a.frozen for a in accounts())
    assert open_account(accounts(), "d1", "dan")[-1].frozen is False


def test_hidden_same():
    assert attempt(accounts(), "a1", "a1", 5) == "declined: same account"
