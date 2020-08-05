from typing import Tuple, Iterator, Dict, List, NamedTuple
import os

from .. import api, storage


class SolvedRequest(NamedTuple):
    request: api.Request
    spec: api.Spec
    repo: storage.Repository


class Solution:
    """Represents a set of resolved packages."""

    def __init__(self, options: api.OptionMap = None) -> None:

        self._options = api.OptionMap(options or {})
        self._resolved: Dict[api.Request, Tuple[api.Spec, storage.Repository]] = {}

    def __bool__(self) -> bool:
        return bool(self._resolved)

    def __contains__(self, other: api.Request) -> bool:

        return other in self._resolved

    def __len__(self) -> int:
        return len(self._resolved)

    def options(self) -> api.OptionMap:
        """Return the options used to generate this solution."""
        return api.OptionMap(self._options)

    def set_options(self, options: api.OptionMap) -> None:
        """Update the options used for this solution to the given ones."""
        self._options = api.OptionMap(options)

    def repositories(self) -> List[storage.Repository]:
        """Return the set of repositories in this solution."""

        repos = []
        for _, _, repo in self.items():
            if repo not in repos:
                repos.append(repo)
        return repos

    def clone(self) -> "Solution":

        other = Solution(self._options)
        other._resolved.update(self._resolved)
        return other

    def add(
        self, request: api.Request, package: api.Spec, source: storage.Repository
    ) -> None:

        self._resolved[request] = (package, source)

    def update(self, other: "Solution") -> None:
        for request, spec, repo in other.items():
            self.add(request, spec, repo)

    def items(self) -> Iterator[SolvedRequest]:

        for request, (spec, repo) in self._resolved.items():
            yield SolvedRequest(request, spec, repo)

    def remove(self, name: str) -> None:

        for request in self._resolved:
            if request.pkg.name == name:
                break
        else:
            raise KeyError(name)

        del self._resolved[request]

    def get(self, name: str) -> SolvedRequest:

        for request in self._resolved:
            if request.pkg.name == name:
                return SolvedRequest(request, *self._resolved[request])
        raise KeyError(name)

    def to_environment(self, base: Dict[str, str] = None) -> Dict[str, str]:
        """Return the data of this solution as environment variables.

        If base is not given, use current os environment.
        """

        if base is None:
            base = dict(os.environ)
        else:
            base = base.copy()

        base["SPK_ACTIVE_PREFIX"] = "/spfs"
        for solved in self.items():

            spec = solved.spec
            base[f"SPK_PKG_{spec.pkg.name}"] = str(spec.pkg)
            base[f"SPK_PKG_{spec.pkg.name}_VERSION"] = str(spec.pkg.version)
            base[f"SPK_PKG_{spec.pkg.name}_BUILD"] = str(spec.pkg.build)
            base[f"SPK_PKG_{spec.pkg.name}_VERSION_MAJOR"] = str(spec.pkg.version.major)
            base[f"SPK_PKG_{spec.pkg.name}_VERSION_MINOR"] = str(spec.pkg.version.minor)
            base[f"SPK_PKG_{spec.pkg.name}_VERSION_PATCH"] = str(spec.pkg.version.patch)
            base[f"SPK_PKG_{spec.pkg.name}_VERSION_BASE"] = api.VERSION_SEP.join(
                str(p) for p in spec.pkg.version.parts
            )

        return base
