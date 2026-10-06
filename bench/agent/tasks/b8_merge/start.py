from typing import Any, Optional

# A JSON value is None, bool, int, str, a list of JSON values, or a dict from str to JSON values.
J = Any


def render(j: J) -> str:
    if j is None:
        return "null"
    if isinstance(j, bool):
        return "true" if j else "false"
    if isinstance(j, int):
        return str(j)
    if isinstance(j, str):
        return '"' + j + '"'
    if isinstance(j, list):
        return "[" + ",".join(render(x) for x in j) + "]"
    return "{" + ",".join('"' + k + '":' + render(v) for k, v in j.items()) + "}"


def get_field(j: J, key: str) -> Optional[J]:
    if isinstance(j, dict):
        return j.get(key)
    return None


def get_path(j: J, path: str) -> Optional[J]:
    cur = j
    for k in path.split("."):
        if cur is None:
            return None
        cur = get_field(cur, k)
    return cur


def merge(target: J, patch: J) -> J:
    if isinstance(target, dict) and isinstance(patch, dict):
        out = {k: v for k, v in target.items() if k not in patch}
        out.update(patch)
        return out
    return patch


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
