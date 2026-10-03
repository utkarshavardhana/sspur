import pytest
from app import *


def cfg():
    return parse(" host = example.com ; port=8080;# port=9;url=http://x?a=b;empty=;=nokey;junk;port = 9090;debug=Yes;n=-5;bad=12a;off=NO")


def test_hidden_len():
    assert len(cfg()) == 9


def test_hidden_keys():
    assert keys(cfg()) == ["host", "port", "url", "empty", "debug", "n", "bad", "off"]


def test_hidden_trim():
    assert lookup(cfg(), "host") == "example.com"


def test_hidden_first_eq():
    assert lookup(cfg(), "url") == "http://x?a=b"


def test_hidden_empty_value():
    assert lookup(cfg(), "empty") == ""


def test_hidden_last_wins():
    assert get_int(cfg(), "port") == 9090


def test_hidden_neg():
    assert get_int(cfg(), "n") == -5


def test_hidden_not_int():
    with pytest.raises(NotInt) as e:
        get_int(cfg(), "bad")
    assert (e.value.key, e.value.value) == ("bad", "12a")
    assert isinstance(e.value, CfgErr)


def test_hidden_int_missing():
    with pytest.raises(Missing) as e:
        get_int(cfg(), "zz")
    assert e.value.key == "zz"


def test_hidden_bool():
    assert get_bool(cfg(), "debug", False) is True
    assert get_bool(cfg(), "off", True) is False


def test_hidden_bool_default():
    assert get_bool(cfg(), "zz", True) is True
    assert get_bool(cfg(), "zz", False) is False


def test_hidden_not_bool():
    with pytest.raises(NotBool) as e:
        get_bool(cfg(), "host", False)
    assert (e.value.key, e.value.value) == ("host", "example.com")
    assert isinstance(e.value, CfgErr)


def test_hidden_blank():
    assert [e.key for e in parse("a=1;;b=2; ;#c=3")] == ["a", "b"]
