from enum import Enum


class Kind(Enum):
    A = 1
    B = 2


class Order:
    number: int
    removed: int

    def ship(self, days: int) -> None: ...

    def drop(self) -> None: ...
