from typing import IO
import os
import io
import stat
import uuid
import shutil
import hashlib

import structlog

from ... import tracking

_CHUNK_SIZE = 1024

_logger = structlog.get_logger(__name__)


class BlobStorage:
    def __init__(self, root: str) -> None:

        self._root = os.path.abspath(root)

    def open_blob(self, digest: str) -> IO[bytes]:
        """Return a handle to the blob identified by the given digest.

        Raises:
            ValueError: if the blob does not exist in this storage
        """
        filepath = os.path.join(self._root, digest)
        try:
            return open(filepath, "rb")
        except FileNotFoundError:
            raise ValueError("Unknown blob: " + digest)

    def write_blob(self, data: IO[bytes]) -> str:
        """Read the given data stream to completion, and store as a blob.

        Return the digest of the stored blob.
        """

        os.makedirs(self._root, exist_ok=True)

        hasher = hashlib.sha256()
        working_filename = "work-" + uuid.uuid1().hex
        working_filepath = os.path.join(self._root, working_filename)
        with open(working_filepath, "xb") as working_file:
            chunk = data.read(_CHUNK_SIZE)
            hasher.update(chunk)
            working_file.write(chunk)

        digest = hasher.hexdigest()
        final_filepath = os.path.join(self._root, digest)
        try:
            os.rename(working_filepath, final_filepath)
            os.chmod(final_filepath, 0o444)
        except FileExistsError:
            _logger.debug("blob already exists", digest=digest)
            os.remove(working_filepath)

        return digest

    def commit_dir(self, dirname: str) -> tracking.Manifest:

        working_dirname = "work-" + uuid.uuid1().hex
        working_dirpath = os.path.join(self._root, working_dirname)

        _logger.info("copying file tree")
        shutil.copytree(dirname, working_dirpath, symlinks=True)

        _logger.info("computing file manifest")
        manifest = tracking.compute_manifest(working_dirpath)

        _logger.info("committing file manifest")
        for rendered_path, entry in manifest.walk_abs(working_dirpath):

            if entry.kind is tracking.EntryKind.TREE:
                continue

            committed_path = os.path.join(self._root, entry.digest)
            if stat.S_ISLNK(entry.mode):
                data = os.readlink(rendered_path)
                stream = io.BytesIO(data.encode("utf-8"))
                digest = self.write_blob(stream)
                assert digest == entry.digest, "symlink did not match expected digest"
                continue

            try:
                os.rename(rendered_path, committed_path)
                os.chmod(committed_path, 0o444)
            except FileExistsError:
                _logger.debug("file exists", digest=entry.digest)
                os.remove(rendered_path)

        _logger.info("committing rendered manifest")
        rendered_dirpath = os.path.join(self._root, manifest.digest)
        try:
            os.rename(working_dirpath, rendered_dirpath)
        except FileExistsError:
            shutil.rmtree(working_dirpath)
        self.render_manifest(manifest)

        return manifest

    def render_manifest(self, manifest: tracking.Manifest) -> str:

        rendered_dirpath = os.path.join(self._root, manifest.digest)

        for rendered_path, entry in manifest.walk_abs(rendered_dirpath):
            if entry.kind is tracking.EntryKind.TREE:
                os.makedirs(rendered_path, exist_ok=True)
            elif entry.kind is tracking.EntryKind.BLOB:
                committed_path = os.path.join(self._root, entry.digest)

                if stat.S_ISLNK(entry.mode):

                    try:
                        with open(committed_path, "r") as f:
                            target = f.read()
                    except FileNotFoundError:
                        raise ValueError("Unknown blob: " + entry.digest)
                    try:
                        os.symlink(target, rendered_path)
                    except FileExistsError:
                        pass
                    continue

                try:
                    os.link(committed_path, rendered_path)
                except FileExistsError:
                    pass
                except FileNotFoundError:
                    raise ValueError("Unknown blob: " + entry.digest)
            else:
                raise NotImplementedError(f"Unsupported entry kind: {entry.kind}")

        for rendered_path, entry in reversed(list(manifest.walk_abs(rendered_dirpath))):
            os.chmod(rendered_path, entry.mode)

        return rendered_dirpath
