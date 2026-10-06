from typing import Any, List, Optional

# A JSON value is None, bool, int, str, a list of JSON values, or a dict from str to JSON values.
J = Any


class ConfigErr(Exception):
    pass


class MissingKey(ConfigErr):
    def __init__(self, path: str):
        super().__init__(path)
        self.path = path


def _quote(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def render(j: J) -> str:
    if j is None:
        return "null"
    if isinstance(j, bool):
        return "true" if j else "false"
    if isinstance(j, int):
        return str(j)
    if isinstance(j, str):
        return _quote(j)
    if isinstance(j, list):
        return "[" + ",".join(render(x) for x in j) + "]"
    return "{" + ",".join(_quote(k) + ":" + render(v) for k, v in j.items()) + "}"


_MISSING = object()


def _get(j: J, key: str) -> Any:
    if isinstance(j, dict):
        return j.get(key, _MISSING)
    if isinstance(j, list) and key.isdigit():
        i = int(key)
        return j[i] if i < len(j) else _MISSING
    return _MISSING


def get_field(j: J, key: str) -> Optional[J]:
    v = _get(j, key)
    return None if v is _MISSING else v


def _find(j: J, path: str) -> Any:
    cur = j
    for k in path.split("."):
        cur = _get(cur, k)
        if cur is _MISSING:
            return _MISSING
    return cur


def get_path(j: J, path: str) -> Optional[J]:
    v = _find(j, path)
    return None if v is _MISSING else v


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


def merge_all(base: J, patches: List[J]) -> J:
    for p in patches:
        base = merge(base, p)
    return base


def require(j: J, paths: List[str]) -> None:
    for path in paths:
        if _find(j, path) is _MISSING:
            raise MissingKey(path)


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


def test_keeps_order():
    assert render(merge(defaults(), {"port": 1, "tags": None})) == '{"name":"app","port":1,"db":{"host":"localhost","pool":5}}'


def test_required():
    try:
        require(defaults(), ["db.host", "db.port"])
        assert False
    except MissingKey as e:
        assert e.path == "db.port"
