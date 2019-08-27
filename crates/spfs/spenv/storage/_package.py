from typing import NamedTuple, Tuple, List
import os
import enum
import uuid
import stat
import errno
import shutil
import hashlib

from .. import tracking
from ._layer import Layer


class Package(Layer):
    """Packages represent a logical collection of software artifacts.

    Packages are considered completely immutable, and are
    uniquely identifyable by the computed hash of all
    relevant file and metadata.
    """

    _diffdir = "diff"
    _metadir = "meta"
    dirs = (_diffdir, _metadir)

    def __init__(self, root: str) -> None:
        """Create a new instance to represent the package data at 'root'."""
        self._root = os.path.abspath(root)

    def __repr__(self) -> str:
        return f"Package('{self.rootdir}')"

    @property
    def ref(self) -> str:
        """Return the identifying reference of this package.

        This is usually the hash string of all relevant file and metadata.
        """
        return os.path.basename(self._root)

    @property
    def rootdir(self) -> str:
        """Return the root directory where this package is stored."""
        return self._root

    @property
    def diffdir(self) -> str:
        """Return the directory in which file data is stored."""
        return os.path.join(self._root, self._diffdir)

    @property
    def metadir(self) -> str:
        """Return the directory in which the metadata is stored."""
        return os.path.join(self._root, self._metadir)

    def read_manifest(self) -> tracking.Manifest:
        """Read the cached file manifest of this package."""
        reader = tracking.ManifestReader(self.diffdir)
        return reader.read()

    def compute_manifest(self) -> tracking.Manifest:
        """Compute the file manifest of this package.

        All file data must be hashed, which can be a heavy operation.
        In most cases, reading the cached manifest is more appropriate,
        as package data is considered immutable.
        """
        return tracking.compute_manifest(self.diffdir)


def _ensure_package(path: str) -> Package:

    os.makedirs(path, exist_ok=True, mode=0o777)
    for subdir in Package.dirs:
        os.makedirs(os.path.join(path, subdir), exist_ok=True, mode=0o777)
    return Package(path)


class PackageStorage:
    """Manages the on-disk storage of packages."""

    def __init__(self, root: str) -> None:
        """Initialize a new storage inside the given root directory."""
        self._root = os.path.abspath(root)

    def read_package(self, ref: str) -> Package:
        """Read package information from this storage.

        Args:
            ref (str): The identifier for the package to read.

        Raises:
            ValueError: If the package does not exist.

        Returns:
            Package: The package data.
        """

        package_path = os.path.join(self._root, ref)
        if not os.path.exists(package_path):
            raise ValueError(f"Unknown package: {ref}")
        return Package(package_path)

    def _ensure_package(self, ref: str) -> Package:

        package_dir = os.path.join(self._root, ref)
        return _ensure_package(package_dir)

    def remove_package(self, ref: str) -> None:
        """Remove a package from this storage.

        Args:
            ref (str): The identifier for the package to remove.

        Raises:
            ValueError: If the package does not exist.
        """

        dirname = os.path.join(self._root, ref)
        try:
            shutil.rmtree(dirname)
        except OSError as e:
            if e.errno == errno.ENOENT:
                raise ValueError("Unknown package: " + ref)
            raise

    def list_packages(self) -> List[Package]:
        """List all stored packages.

        Returns:
            List[Package]: The stored packages.
        """

        try:
            dirs = os.listdir(self._root)
        except OSError as e:
            if e.errno == errno.ENOENT:
                dirs = []
            else:
                raise

        return [Package(os.path.join(self._root, d)) for d in dirs]

    def commit_dir(self, dirname: str) -> Package:
        """Create a package from the contents of a directory."""

        tmp_package = self._ensure_package("work-" + uuid.uuid1().hex)
        os.rmdir(tmp_package.diffdir)
        shutil.copytree(dirname, tmp_package.diffdir, symlinks=True)

        manifest = tmp_package.compute_manifest()
        tree = manifest.get_path(tmp_package.diffdir)
        assert tree is not None, "Manifest must have entry for package diffdir"

        writer = tracking.ManifestWriter(tmp_package.metadir)
        writer.rewrite(manifest)

        new_root = os.path.join(self._root, tree.digest)
        try:
            os.rename(tmp_package._root, new_root)
        except OSError as e:
            self.remove_package(tmp_package.ref)
            if e.errno in (errno.EEXIST, errno.ENOTEMPTY):
                pass
            else:
                raise
        return self.read_package(tree.digest)
