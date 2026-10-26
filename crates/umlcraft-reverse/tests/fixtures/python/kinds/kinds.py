from abc import ABC, ABCMeta
from enum import Enum, IntEnum
from typing import Protocol


class Plain:
    pass


class Shape(Protocol):
    def area(self) -> float: ...


class Color(Enum):
    RED = 1
    GREEN = 2


class Level(IntEnum):
    LOW = 1


class Base(ABC):
    pass


class Meta(metaclass=ABCMeta):
    pass
