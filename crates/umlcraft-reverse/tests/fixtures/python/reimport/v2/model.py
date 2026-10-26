from enum import Enum


class Kind(Enum):
    A = 1
    B = 2
    C = 3


class Order:
    number: str
    added: int

    def ship(self, days: int) -> None: ...

    def ship_later(self, days: int) -> None: ...
