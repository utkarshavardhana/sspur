from typing import Generic, Optional, TypeVar

T = TypeVar("T")


class RingBuffer(Generic[T]):
    def __init__(self, capacity: int):
        self._buf: list[Optional[T]] = [None] * capacity
        self._head = 0
        self._len = 0

    def __len__(self) -> int:
        return self._len

    def push(self, x: T) -> bool:
        if self._len == len(self._buf):
            return False
        self._buf[(self._head + self._len) % len(self._buf)] = x
        self._len += 1
        return True

    def pop(self) -> Optional[T]:
        if self._len == 0:
            return None
        x = self._buf[self._head]
        self._buf[self._head] = None
        self._head = (self._head + 1) % len(self._buf)
        self._len -= 1
        return x
