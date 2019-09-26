from typing import Callable
import uuid
import logging

import pytest
import py.path
import structlog

import spenv

logging.basicConfig()
logging.getLogger().setLevel(logging.DEBUG)
structlog.configure(
    processors=[
        structlog.stdlib.add_log_level,
        structlog.stdlib.PositionalArgumentsFormatter(),
        structlog.processors.StackInfoRenderer(),
        structlog.processors.format_exc_info,
        structlog.dev.ConsoleRenderer(),
    ],
    logger_factory=structlog.stdlib.LoggerFactory(),
    wrapper_class=structlog.stdlib.BoundLogger,
)


@pytest.fixture
def tmprepo(tmpdir: py.path.local) -> spenv.storage.fs.Repository:

    root = tmpdir.join("tmprepo").ensure(dir=True)
    return spenv.storage.fs.Repository(root.strpath)


@pytest.fixture(autouse=True)
def config(tmpdir: py.path.local) -> spenv.Config:

    spenv._config._CONFIG = spenv.Config()
    spenv._config._CONFIG.read_string(
        f"""
[storage]
root = {tmpdir.join('storage_root').strpath}

[remote.origin]
address = file://{tmpdir.join('remote_origin').strpath}
"""
    )
    return spenv.get_config()
