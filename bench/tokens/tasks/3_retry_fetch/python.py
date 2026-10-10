import time
from dataclasses import dataclass

import httpx


@dataclass(frozen=True)
class User:
    id: str
    name: str


class FetchError(Exception):
    pass


def fetch_user(url: str, attempts: int = 5) -> User:
    for n in range(attempts):
        try:
            r = httpx.get(url, timeout=2.0)
            if r.status_code < 500:
                r.raise_for_status()
                d = r.json()
                return User(id=d["id"], name=d["name"])
        except httpx.TransportError:
            pass
        except httpx.HTTPStatusError as e:
            raise FetchError(str(e)) from e
        time.sleep(0.1 * 2**n)
    raise FetchError(f"gave up after {attempts} attempts")
