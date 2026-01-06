from typing import Dict, Generic, TypeVar

_KT = TypeVar("_KT")
_VT = TypeVar("_VT")

class TTLCache(Dict[_KT, _VT], Generic[_KT, _VT]):
    def __init__(self, maxsize: int, ttl: int) -> None: ...
