from typing import List

from . import api, build, storage, solve, exec, io, test

EMPTY_DIGEST: Digest

class Digest: ...

class Runtime:
    def get_stack(self) -> List[Digest]: ...

def version() -> str: ...
def configure_logging(verbosity: int) -> None: ...
def active_runtime() -> Runtime: ...
def reconfigure_runtime(
    editable: bool = None,
    reset: List[str] = None,
    stack: List[Digest] = None,
) -> None: ...
def build_shell_initialized_command(cmd: str, *args: str) -> List[str]: ...
def build_interactive_shell_command() -> List[str]: ...
def commit_layer(runtime: Runtime) -> Digest: ...
def render_into_dir(stack: List[Digest], path: str) -> None: ...
