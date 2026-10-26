from typing import ClassVar, Final, Optional


class Line:
    pass


class Hints:
    untyped = 1
    count: ClassVar[int] = 0
    limit: Final[int] = 10
    frozen: Final = "x"
    line: Line
    maybe: Optional[Line]
    union: Line | None
    either: int | str
    forward: "Line"
    forward_list: "list[Line]"
    callback: "Callable[[int], str]"

    def apply(self, x, y: int, z: "Line" = None) -> "Hints": ...
