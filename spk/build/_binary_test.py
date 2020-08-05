from typing import Any
import os

import pytest
import py.path

import spfs

from .. import api, storage
from ._binary import (
    validate_build_changeset,
    BuildError,
    BinaryPackageBuilder,
)


def test_validate_build_changeset_nothing() -> None:

    with pytest.raises(BuildError):

        validate_build_changeset([])


def test_validate_build_changeset_modified() -> None:

    with pytest.raises(BuildError):

        validate_build_changeset(
            [
                spfs.tracking.Diff(
                    path="/spfs/file.txt", mode=spfs.tracking.DiffMode.changed
                )
            ]
        )


def test_build_artifacts(tmpdir: py.path.local, capfd: Any, monkeypatch: Any) -> None:

    spec = api.Spec.from_dict(
        {"pkg": "test/1.0.0", "build": {"script": "echo $PWD > /dev/stderr"}}
    )

    (
        BinaryPackageBuilder()
        .from_spec(spec)
        .with_source(tmpdir.strpath)
        ._build_artifacts()
    )

    _, err = capfd.readouterr()
    assert err.strip() == tmpdir.strpath


def test_build_package_options(tmprepo: storage.SpFSRepository) -> None:

    dep_spec = api.Spec.from_dict(
        {"pkg": "dep/1.0.0", "build": {"script": "touch /spfs/dep-file"}}
    )
    spec = api.Spec.from_dict(
        {
            "pkg": "top/1.2.3+r.2",
            "build": {
                "script": [
                    "touch /spfs/top-file",
                    "test -f /spfs/dep-file",
                    "env | grep SPK",
                    'test ! -x "$SPK_PKG_dep"',
                    'test "$SPK_PKG_dep_VERSION" == "1.0.0"',
                    'test "$SPK_OPT_dep" == "1.0.0"',
                    'test "$SPK_PKG_NAME" == "top"',
                    'test "$SPK_PKG_VERSION" == "1.2.3+r.2"',
                    'test "$SPK_PKG_VERSION_MAJOR" == "1"',
                    'test "$SPK_PKG_VERSION_MINOR" == "2"',
                    'test "$SPK_PKG_VERSION_PATCH" == "3"',
                    'test "$SPK_PKG_VERSION_BASE" == "1.2.3"',
                ],
                "options": [{"pkg": "dep"}],
            },
        }
    )

    tmprepo.publish_spec(dep_spec)
    BinaryPackageBuilder.from_spec(dep_spec).with_source(".").with_repository(
        tmprepo
    ).build()
    spec = (
        BinaryPackageBuilder.from_spec(spec)
        .with_source(".")
        .with_repository(tmprepo)
        .with_option("dep", "2.0.0")  # option should be set in final published spec
        .with_option("top.dep", "1.0.0")  # specific option takes precendence
        .build()
    )
    build_options = tmprepo.read_spec(spec.pkg).build.resolve_all_options(
        api.OptionMap({"dep": "7"})  # given value should be ignored after build
    )
    assert build_options.get("dep") == "~1.0.0"


def test_build_package_pinning(tmprepo: storage.SpFSRepository) -> None:

    dep_spec = api.Spec.from_dict(
        {"pkg": "dep/1.0.0", "build": {"script": "touch /spfs/dep-file"}}
    )
    spec = api.Spec.from_dict(
        {
            "pkg": "top/1.0.0",
            "build": {
                "script": ["touch /spfs/top-file",],
                "options": [{"pkg": "dep", "default": "1.0.0"}],
            },
            "install": {"requirements": [{"pkg": "dep", "fromBuildEnv": "~x.x"}]},
        }
    )

    tmprepo.publish_spec(dep_spec)
    BinaryPackageBuilder.from_spec(dep_spec).with_source(os.getcwd()).with_repository(
        tmprepo
    ).build()
    spec = (
        BinaryPackageBuilder.from_spec(spec)
        .with_source(os.getcwd())
        .with_repository(tmprepo)
        .build()
    )

    spec = tmprepo.read_spec(spec.pkg)
    assert str(spec.install.requirements[0].pkg) == "dep/~1.0"


def test_build_bad_options() -> None:

    spec = api.Spec.from_dict(
        {
            "pkg": "my-package/1.0.0",
            "build": {
                "script": ["touch /spfs/top-file",],
                "options": [{"var": "debug", "choices": ["on", "off"]}],
            },
        }
    )

    with pytest.raises(ValueError):
        spec = (
            BinaryPackageBuilder.from_spec(spec)
            .with_source(os.getcwd())
            .with_option("debug", "false")
            .build()
        )
