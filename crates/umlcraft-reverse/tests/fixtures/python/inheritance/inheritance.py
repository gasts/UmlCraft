import typing


class Base:
    pass


class Mixin:
    pass


class Derived(Base, Mixin):
    pass


class FromModule(typing.NamedTuple):
    x: int


class Explicit(object):
    pass
