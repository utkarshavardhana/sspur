import pytest
from app import *


def h_m(t, p):
    return render(merge(t, p))


def h_cfg():
    return {"name": "app", "servers": [{"host": "a"}, {"host": "b"}], "db": {"pool": 5}}


def h_req(j, paths):
    try:
        require(j, paths)
        return "ok"
    except MissingKey as e:
        return "missing " + e.path
    except ConfigErr:
        return "other"


def test_hidden_rfc_replace():
    assert h_m({"a": "b"}, {"a": "c"}) == '{"a":"c"}'


def test_hidden_rfc_add():
    assert h_m({"a": "b"}, {"b": "c"}) == '{"a":"b","b":"c"}'


def test_hidden_rfc_remove():
    assert h_m({"a": "b"}, {"a": None}) == "{}" and h_m({"a": "b", "b": "c"}, {"a": None}) == '{"b":"c"}'


def test_hidden_rfc_array_replaced():
    assert h_m({"a": ["b"]}, {"a": "c"}) == '{"a":"c"}' and h_m({"a": "c"}, {"a": ["b"]}) == '{"a":["b"]}'


def test_hidden_rfc_nested():
    assert h_m({"a": {"b": "c"}}, {"a": {"b": "d", "c": None}}) == '{"a":{"b":"d"}}'


def test_hidden_rfc_array_of_objects():
    assert h_m({"a": [{"b": "c"}]}, {"a": [1]}) == '{"a":[1]}'


def test_hidden_rfc_non_object_patch():
    assert h_m(["a", "b"], ["c", "d"]) == '["c","d"]' and h_m({"a": "b"}, ["c"]) == '["c"]'
    assert h_m({"a": "foo"}, None) == "null" and h_m({"a": "foo"}, "bar") == '"bar"'


def test_hidden_rfc_keep_target_null():
    assert h_m({"e": None}, {"a": 1}) == '{"e":null,"a":1}'


def test_hidden_rfc_non_object_target():
    assert h_m([1, 2], {"a": "b", "c": None}) == '{"a":"b"}'


def test_hidden_rfc_deep_new():
    assert h_m({}, {"a": {"bb": {"ccc": None}}}) == '{"a":{"bb":{}}}'


def test_hidden_key_order():
    assert h_m({"a": 1, "b": 2, "c": 3}, {"b": 9, "d": 4}) == '{"a":1,"b":9,"c":3,"d":4}'


def test_hidden_defaults_merge():
    assert h_m(defaults(), {"port": 9090, "db": {"pool": 10, "user": "u"}}) == '{"name":"app","port":9090,"db":{"host":"localhost","pool":10,"user":"u"},"tags":["a"]}'


def test_hidden_escape():
    assert render('say "hi" \\ bye') == '"say \\"hi\\" \\\\ bye"' and render({'k"q': 1}) == '{"k\\"q":1}'


def test_hidden_path_index():
    assert get_path(h_cfg(), "servers.1.host") == "b" and get_path(h_cfg(), "servers.0") == {"host": "a"}


def test_hidden_path_missing():
    assert get_path(h_cfg(), "servers.5.host") is None and get_path(h_cfg(), "servers.x") is None
    assert get_path(h_cfg(), "name.first") is None and get_path(h_cfg(), "db.pool") == 5


def test_hidden_merge_all():
    assert render(merge_all(defaults(), [])) == render(defaults())
    assert render(merge_all({"a": 1}, [{"a": 2, "b": 3}, {"b": None}])) == '{"a":2}'


def test_hidden_require():
    assert h_req(h_cfg(), ["name", "db.pool", "servers.1.host"]) == "ok"
    assert h_req(h_cfg(), ["name", "db.user", "x"]) == "missing db.user" and h_req(h_cfg(), []) == "ok"
