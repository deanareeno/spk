from collections import OrderedDict
import os
import random

import py.path
import pytest

from ._manifest import (
    MutableManifest,
    Entry,
    EntryKind,
    compute_tree,
    compute_entry,
    compute_manifest,
)


def test_entry_blobs_compare_name() -> None:

    a = Entry(name="a", kind=EntryKind.BLOB, mode=0, digest="")
    b = Entry(name="b", kind=EntryKind.BLOB, mode=0, digest="")
    assert a < b and b > a


def test_entry_trees_compare_name() -> None:

    a = Entry(name="a", kind=EntryKind.TREE, mode=0, digest="")
    b = Entry(name="b", kind=EntryKind.TREE, mode=0, digest="")
    assert a < b and b > a


def test_entry_compare_kind() -> None:

    blob = Entry(name="a", kind=EntryKind.BLOB, mode=0, digest="")
    tree = Entry(name="b", kind=EntryKind.TREE, mode=0, digest="")
    assert tree > blob and blob < tree


def test_compute_tree_determinism() -> None:

    first = compute_tree("./spenv")
    second = compute_tree("./spenv")
    assert first == second


def test_compute_manifest() -> None:

    root = os.path.abspath("./spenv")
    this = os.path.relpath(__file__, root)
    manifest = compute_manifest(root)
    assert manifest.get_path(this) is not None


def test_manifest_relative_paths(tmpdir: py.path.local) -> None:

    tmpdir.join("dir1.0/dir2.0/file.txt").write("somedata", ensure=True)
    tmpdir.join("dir1.0/dir2.1/file.txt").write("someotherdata", ensure=True)
    tmpdir.join("dir2.0/file.txt").write("evenmoredata", ensure=True)
    tmpdir.join("a_file.txt").write("rootdata", ensure=True)

    manifest = compute_manifest(tmpdir.strpath)
    assert manifest.get_path("/") is not None
    assert manifest.get_path("/dir1.0/dir2.0/file.txt") is not None
    assert manifest.get_path("dir1.0/dir2.1/file.txt") is not None


def test_entry_compare() -> None:

    defaults = {"mode": 0, "digest": ""}
    root_file = Entry(name="file", kind=EntryKind.BLOB, **defaults)  # type: ignore
    root_dir = Entry(name="xdir", kind=EntryKind.TREE, **defaults)  # type: ignore
    assert root_dir > root_file


def test_manifest_sorting(tmpdir: py.path.local) -> None:

    tmpdir.join("dir1.0/dir2.0/file.txt").write("somedata", ensure=True)
    tmpdir.join("dir1.0/dir2.1/file.txt").write("someotherdata", ensure=True)
    tmpdir.join("dir1.0/file.txt").write("thebestdata", ensure=True)
    tmpdir.join("dir2.0/file.txt").write("evenmoredata", ensure=True)
    tmpdir.join("a_file.txt").write("rootdata", ensure=True)
    tmpdir.join("z_file.txt").write("rootdata", ensure=True)

    manifest = MutableManifest(tmpdir.strpath)
    compute_entry(tmpdir.strpath, append_to=manifest)

    items = list(manifest._paths.items())
    random.shuffle(items)
    manifest._paths = OrderedDict(items)

    manifest.sort()
    actual = list(manifest._paths.keys())
    expected = [
        "/",
        "/a_file.txt",
        "/z_file.txt",
        "/dir1.0",
        "/dir1.0/file.txt",
        "/dir1.0/dir2.0",
        "/dir1.0/dir2.0/file.txt",
        "/dir1.0/dir2.1",
        "/dir1.0/dir2.1/file.txt",
        "/dir2.0",
        "/dir2.0/file.txt",
    ]
    assert actual == expected
