from typing import Union, Dict, Any
from dataclasses import dataclass
import base64
import binascii

SRC = "src"


@dataclass
class Build:
    """Build represents a package build identifier."""

    digest: str

    def is_source(self) -> bool:
        return self.digest == SRC

    def __str__(self) -> str:
        return self.digest


def parse_build(digest: str) -> Build:

    if digest == SRC:
        return Build(SRC)

    try:
        base64.b32decode(digest)
    except binascii.Error as e:
        raise ValueError(f"Invalid build digest: {e}") from None
    return Build(digest)
