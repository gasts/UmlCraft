from abc import abstractmethod
from typing import final


class Service:
    @staticmethod
    def create(name: str) -> "Service": ...

    @classmethod
    def default(cls) -> "Service": ...

    @abstractmethod
    def run(self) -> None: ...

    @property
    def name(self) -> str:
        return "service"

    @final
    def stop(self) -> None:
        pass

    async def fetch(self, url: str, *args: int, timeout: float = 1.0, **options) -> bytes:
        return b""

    def positional(self, a, /, b, *, c) -> None: ...
