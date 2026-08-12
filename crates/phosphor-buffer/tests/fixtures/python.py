"""T083 fixture — Python."""

from __future__ import annotations

import asyncio
from dataclasses import dataclass, field
from enum import Enum
from typing import Iterator, Protocol

GLYPHS: dict[str, str] = {"unseen": "●", "thinking": "✻", "ask": "!"}


class Mode(str, Enum):
    NORMAL = "normal"
    INSERT = "insert"


class Query(Protocol):
    def run(self, needle: str, /, *, limit: int = 50) -> list[str]: ...


@dataclass(slots=True, frozen=True)
class Region:
    id: str
    start: int
    end: int
    seen: bool = False
    tags: list[str] = field(default_factory=list)

    def __len__(self) -> int:
        return self.end - self.start


def classify(region: Region) -> str:
    match region:
        case Region(seen=True, tags=[]):
            return "clean"
        case Region(seen=False) as r if len(r) > 100:
            return "big-unseen"
        case Region(tags=[first, *rest]):
            return f"{first}+{len(rest)}"
        case _:
            return "other"


def walk(rows: list[Region]) -> Iterator[tuple[int, str]]:
    for i, row in enumerate(rows):
        if (n := len(row)) > 0:
            yield i, f"{row.id}:{n:>4d}:{GLYPHS['unseen'] if not row.seen else ''}"


async def gather(paths: list[str]) -> dict[str, int]:
    async def one(p: str) -> tuple[str, int]:
        await asyncio.sleep(0)
        return p, len(p)

    return dict(await asyncio.gather(*(one(p) for p in paths)))


class Store:
    def __init__(self, rows: list[Region] | None = None) -> None:
        self._rows = rows or []

    def __enter__(self) -> Store:
        return self

    def __exit__(self, *exc: object) -> bool:
        return False

    @property
    def unseen(self) -> int:
        return sum(1 for r in self._rows if not r.seen)


if __name__ == "__main__":
    with Store([Region("a", 0, 10)]) as store:
        print(store.unseen, [c for _, c in walk(store._rows)])
