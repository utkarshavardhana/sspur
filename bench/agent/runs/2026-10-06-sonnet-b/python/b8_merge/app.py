from typing import Any, Optional

# A JSON value is None, bool, int, str, a list of JSON values, or a dict from str to JSON values.
J = Any


def _q(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def render(j: J) -> str:
    if j is None:
        return "null"
    if isinstance(j, bool):
        return "true" if j else "false"
    if isinstance(j, int):
        return str(j)
    if isinstance(j, str):
        return _q(j)
    if isinstance(j, list):
        return "[" + ",".join(render(x) for x in j) + "]"
    return "{" + ",".join(_q(k) + ":" + render(v) for k, v in j.items()) + "}"


def get_field(j: J, key: str) -> Optional[J]:
    if isinstance(j, dict):
        return j.get(key)
    if isinstance(j, list) and key.isascii() and key.isdigit():
        i = int(key)
        if i < len(j):
            return j[i]
    return None


def get_path(j: J, path: str) -> Optional[J]:
    cur = j
    for k in path.split("."):
        if cur is None:
            return None
        cur = get_field(cur, k)
    return cur


def merge(target: J, patch: J) -> J:
    if not isinstance(patch, dict):
        return patch
    out = dict(target) if isinstance(target, dict) else {}
    for k, v in patch.items():
        if v is None:
            out.pop(k, None)
        else:
            out[k] = merge(out.get(k), v)
    return out


def merge_all(base: J, patches: list) -> J:
    for p in patches:
        base = merge(base, p)
    return base


class ConfigErr(Exception):
    pass


class MissingKey(ConfigErr):
    def __init__(self, path: str):
        super().__init__(path)
        self.path = path


def require(j: J, paths: list) -> None:
    for p in paths:
        if get_path(j, p) is None:
            raise MissingKey(p)


def defaults() -> J:
    return {"name": "app", "port": 8080, "db": {"host": "localhost", "pool": 5}, "tags": ["a"]}


if __name__ == "__main__":
    print(render(merge(defaults(), {"port": 9090})))


def test_renders():
    assert render({"a": [1, True, None]}) == '{"a":[1,true,null]}'


def test_path():
    assert get_path(defaults(), "db.pool") == 5 and get_path(defaults(), "db.user") is None


def test_merges_top_level():
    assert get_path(merge(defaults(), {"name": "web"}), "name") == "web"


def test_rfc7396():
    t = {"a": "b", "c": {"d": "e", "f": "g"}}
    assert merge(t, {"a": "z", "c": {"f": None}}) == {"a": "z", "c": {"d": "e"}}
    assert merge({"a": 1}, {"a": [1]}) == {"a": [1]}
    assert merge({"a": [1, 2]}, {"a": [3]}) == {"a": [3]}
    assert merge("x", {"a": {"b": None, "c": 1}}) == {"a": {"c": 1}}
    assert merge({"a": 1}, [1]) == [1] and merge({"a": 1}, 5) == 5
    assert merge({"a": 1}, {"a": None, "b": None}) == {}


def test_key_order():
    r = merge({"b": 1, "a": 2}, {"c": 3, "a": 9, "0": 1})
    assert list(r) == ["b", "a", "c", "0"] and render(r) == '{"b":1,"a":9,"c":3,"0":1}'


def test_escaping():
    assert render({'k"\\': 'v"\\'}) == '{"k\\"\\\\":"v\\"\\\\"}'


def test_array_paths():
    j = {"servers": [{"host": "a"}, {"host": "b"}]}
    assert get_path(j, "servers.1.host") == "b"
    assert get_path(j, "servers.2.host") is None
    assert get_path(j, "servers.x") is None and get_path(j, "servers.-1") is None


def test_merge_all():
    assert merge_all({"a": 1}, [{"b": 2}, {"a": None}, {"b": 3}]) == {"b": 3}
    assert merge_all({"a": 1}, []) == {"a": 1}


def test_require():
    import pytest
    require(defaults(), ["name", "db.pool"])
    with pytest.raises(MissingKey) as e:
        require(defaults(), ["name", "x.y", "z"])
    assert e.value.path == "x.y" and isinstance(e.value, ConfigErr)
