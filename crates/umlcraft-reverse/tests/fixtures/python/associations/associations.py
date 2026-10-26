from collections import deque
from typing import Dict, List, Optional


class Line:
    pass


class Customer:
    pass


class Order:
    customer: Customer
    payer: Optional[Customer]
    reader: Customer | None
    lines: list[Line]
    legacy: List[Line]
    queue: deque[Line]
    tags: set[Line]
    by_name: dict[str, Line]
    old_map: Dict[str, Line]
    untyped = None
    note: str
    count: int

    def __init__(self) -> None:
        self.unannotated = Customer()
