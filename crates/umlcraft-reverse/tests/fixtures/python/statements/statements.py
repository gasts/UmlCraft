"""Statements outside classes."""
import os

VERSION = 1


def helper() -> None:
    pass


class Kept:
    print("in the class body")


if __name__ == "__main__":
    helper()
