from typing import Any, Dict, Mapping, Optional

class RequestException(Exception): ...

class Response:
    status_code: int

    def json(self) -> Any: ...
    def raise_for_status(self) -> None: ...

def get(
    url: str,
    params: Optional[Mapping[str, Any]] = ...,
    timeout: Optional[float] = ...,
) -> Response: ...
