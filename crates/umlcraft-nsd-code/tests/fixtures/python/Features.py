"""Python only."""
import os
from typing import Iterator


def area(width: float, height: float = 1.0) -> float:
    """The area of a rectangle."""
    return width * height


def scale(points, factor=2, *args, **kwargs):
    total = area(2, 3)
    a = b = 0
    a, b = b, a
    count: int = 0
    names: list
    count **= 2
    count //= 3
    print(total)
    squares = [p * factor for p in points if p > 0]
    if (n := len(squares)) > 10:
        del squares[0]
    global LIMIT
    assert factor > 0, "positive"
    return a, b


class Shop:
    """A shop."""

    def __init__(self, name):
        self.name = name
        self.items = []

    @staticmethod
    def make(name):
        return Shop(name)

    @classmethod
    def load(cls, path):
        with open(path) as f, lock:
            data = f.read()
        return cls.make(data)

    def add(self, item):
        self.items.append(item)
        self.check()

    def check(self):
        for i in range(len(self.items) - 1, -1, -1):
            if self.items[i] is None:
                break
        else:
            print("all set")
        while self.items:
            self.items.pop()
        else:
            pass

    def kind(self, value):
        match value:
            case Point(x=0) if value.y > 0:
                return "axis"
            case [first, *rest]:
                return "list"
            case _:
                raise ValueError("value") from None

    def risky(self):
        try:
            self.add(1)
        except* (TypeError, ValueError) as group:
            print(group)
        except:
            raise
        else:
            self.check()

    def numbers(self, n) -> Iterator[int]:
        for i in range(n):
            if i > 100:
                return
            yield i

    def ranges(self, n, m):
        for j in range(1, m if n else n + 1):
            print(j)
        try:
            self.check()
        except ValueError:
            print("bad")

    async def fetch(self, url):
        def helper(x):
            return x * 2

        class Local:
            pass

        response = await client.get(url)
        handler = lambda r: r.json()
        return handler(response)

    def broken(self):
        x = = 1
        return x


# Run as a script.
shop = Shop("demo")
if __name__ == "__main__":
    shop.add(area(1, 2))
