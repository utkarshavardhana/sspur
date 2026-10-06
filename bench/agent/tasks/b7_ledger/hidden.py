import pytest
from app import *


def h_add(l, id, ps):
    try:
        return f"ok {len(add_txn(l, Txn(id, 't', ps)).txns)}"
    except UnknownAccount as e:
        return f"unknown {e.name}"
    except Unbalanced as e:
        return f"unbalanced {e.id} {e.diff}"
    except TooFewPostings as e:
        return f"few {e.id}"
    except DuplicateId as e:
        return f"dup {e.id}"
    except Overdrawn as e:
        return f"overdrawn {e.account} {e.balance}"
    except LedgerErr:
        return "other"


def h_sample_add(id, ps):
    return h_add(sample(), id, ps)


def h_rev(id, new_id):
    try:
        return f"ok {balance(reverse(sample(), id, new_id), 'assets:bank')}"
    except UnknownTxn as e:
        return f"no txn {e.id}"
    except DuplicateId as e:
        return f"dup {e.id}"
    except Overdrawn as e:
        return f"overdrawn {e.account} {e.balance}"
    except LedgerErr:
        return "other"


def test_hidden_sample():
    assert balance(sample(), "assets:bank") == 970 and len(sample().txns) == 2


def test_hidden_unbalanced():
    assert h_add(new_ledger(), 1, [p("assets:bank", 10), p("equity:owner", -9)]) == "unbalanced 1 1"
    assert h_add(new_ledger(), 4, [p("assets:bank", 5), p("equity:owner", -8)]) == "unbalanced 4 -3"


def test_hidden_too_few():
    assert h_add(new_ledger(), 1, [p("assets:bank", 0)]) == "few 1" and h_add(new_ledger(), 2, []) == "few 2"


def test_hidden_unknown():
    assert h_add(new_ledger(), 1, [p("assets:nope", 5), p("equity:owner", -5)]) == "unknown assets:nope"


def test_hidden_check_order():
    assert h_add(new_ledger(), 1, [p("zzz", 5)]) == "few 1"
    assert h_add(new_ledger(), 1, [p("zzz", 5), p("equity:owner", -1)]) == "unknown zzz"


def test_hidden_duplicate():
    assert h_sample_add(2, [p("expenses:food", 5), p("assets:bank", -5)]) == "dup 2"


def test_hidden_overdrawn():
    assert h_sample_add(3, [p("expenses:food", 2000), p("assets:bank", -2000)]) == "overdrawn assets:bank -1030"


def test_hidden_overdrawn_cash():
    assert h_sample_add(3, [p("assets:cash", -1), p("assets:bank", 1)]) == "overdrawn assets:cash -1"


def test_hidden_exact_zero():
    assert h_sample_add(3, [p("expenses:food", 970), p("assets:bank", -970)]) == "ok 3"


def test_hidden_other_negative():
    assert h_sample_add(3, [p("assets:cash", 50), p("income:salary", -50)]) == "ok 3"


def test_hidden_trial():
    assert [tuple(x) for x in trial_balance(sample())] == [("assets:bank", 970), ("equity:owner", -1000), ("expenses:food", 30)]


def test_hidden_trial_empty():
    assert list(trial_balance(new_ledger())) == []


def test_hidden_reverse():
    assert h_rev(2, 3) == "ok 1000"


def test_hidden_reverse_txn():
    l = reverse(sample(), 2, 3)
    assert l.txns[-1] == Txn(3, "reverse 2", [p("expenses:food", -30), p("assets:bank", 30)])
    assert [tuple(x) for x in trial_balance(l)] == [("assets:bank", 1000), ("equity:owner", -1000)]


def test_hidden_reverse_errors():
    assert h_rev(9, 3) == "no txn 9" and h_rev(2, 1) == "dup 1" and h_rev(1, 3) == "overdrawn assets:bank -30"


def test_hidden_history():
    assert [tuple(x) for x in history(sample(), "assets:bank")] == [(1, 1000), (2, 970)]
    assert [tuple(x) for x in history(sample(), "expenses:food")] == [(2, 30)]
    assert list(history(sample(), "assets:cash")) == []


def test_hidden_history_net():
    l = add_txn(sample(), Txn(7, "x", [p("assets:bank", 5), p("assets:bank", 5), p("equity:owner", -10)]))
    assert [tuple(x) for x in history(l, "assets:bank")] == [(1, 1000), (2, 970), (7, 980)]
