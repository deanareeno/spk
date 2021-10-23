# Copyright (c) 2021 Sony Pictures Imageworks, et al.
# SPDX-License-Identifier: Apache-2.0
# https://github.com/imageworks/spk

import os
import subprocess
import tempfile
from typing import Iterable, List, Optional

import spkrs

from .. import api, storage, solve, exec, build
from ._build import TestError


class PackageInstallTester:
    def __init__(self, spec: api.Spec, script: str) -> None:
        self._prefix = "/spfs"
        self._spec = spec
        self._script = script
        self._repos: List[storage.Repository] = []
        self._options = api.OptionMap()
        self._additional_requirements: List[api.Request] = []
        self._source: Optional[str] = None
        self._last_solve_graph = solve.Graph()

    def get_solve_graph(self) -> solve.Graph:
        """Return the solver graph for the test environment.

        This is most useful for debugging test environments that failed to resolve,
        and test that failed with a SolverError.

        If the tester has not run, return an incomplete graph.
        """

        return self._last_solve_graph

    def with_option(self, name: str, value: str) -> "PackageInstallTester":

        self._options[name] = value
        return self

    def with_options(self, options: api.OptionMap) -> "PackageInstallTester":

        self._options.update(options)
        return self

    def with_repository(self, repo: storage.Repository) -> "PackageInstallTester":

        self._repos.append(repo)
        return self

    def with_repositories(
        self, repos: Iterable[storage.Repository]
    ) -> "PackageInstallTester":

        self._repos.extend(repos)
        return self

    def with_source(self, source: str) -> "PackageInstallTester":

        self._source = source
        return self

    def with_requirements(
        self, requests: Iterable[api.Request]
    ) -> "PackageInstallTester":

        self._additional_requirements.extend(requests)
        return self

    def test(self) -> None:

        spkrs.reconfigure_runtime(stack=[], editable=True, reset=["*"])

        solver = solve.Solver()
        for request in self._additional_requirements:
            solver.add_request(request)
        solver.update_options(self._options)
        for repo in self._repos:
            solver.add_repository(repo)
        solver.add_request(self._spec.pkg)
        solution = solver.solve()

        layers = exec.resolve_runtime_layers(solution)
        spkrs.reconfigure_runtime(stack=layers)

        env = solution.to_environment(os.environ)
        env["PREFIX"] = self._prefix

        source_dir = "."
        if self._source is not None:
            source_dir = self._source

        with tempfile.NamedTemporaryFile("w+") as script_file:
            script_file.write(self._script)
            script_file.flush()
            os.environ["SHELL"] = "bash"
            cmd = spkrs.build_shell_initialized_command("bash", "-ex", script_file.name)

            with build.deferred_signals():
                proc = subprocess.Popen(cmd, env=env, cwd=source_dir)
                proc.wait()
            if proc.returncode != 0:
                raise TestError(
                    f"Test script returned non-zero exit status: {proc.returncode}"
                )
