"""Finite native result evidence, separate from artifact/runtime authority."""

from dataclasses import dataclass
from typing import Literal


@dataclass(frozen=True)
class NativeSpeechResult:
    text: str
    finish_reason: Literal["stop", "length"]
