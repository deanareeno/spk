# Copyright (c) 2021 Sony Pictures Imageworks, et al.
# SPDX-License-Identifier: Apache-2.0
# https://github.com/imageworks/spk

from typing import Any
import argparse
import sys

import spkrs
import structlog
from colorama import Fore, Style

import spk

from . import _flags

_LOGGER = structlog.get_logger("spk.cli")


def register(
    sub_parsers: argparse._SubParsersAction, **parser_args: Any
) -> argparse.ArgumentParser:

    install_cmd = sub_parsers.add_parser(
        "install",
        aliases=["i"],
        help=_install.__doc__,
        description=_install.__doc__,
        **parser_args,
    )
    install_cmd.add_argument(
        "packages", metavar="PKG", nargs="+", help="The packages to install"
    )
    install_cmd.add_argument(
        "--save",
        nargs="?",
        const="@build",
        metavar="PKG@STAGE",
        help="Also save this requirement to the local package spec file",
    )
    install_cmd.add_argument(
        "--yes",
        "-y",
        action="store_true",
        default=False,
        help="Do not prompt for confirmation, just continue",
    )
    _flags.add_request_flags(install_cmd)
    _flags.add_solver_flags(install_cmd)
    install_cmd.set_defaults(func=_install)
    return install_cmd


def _install(args: argparse.Namespace) -> None:
    """Install a package into spkrs."""

    solver = _flags.get_solver_from_flags(args)
    requests = _flags.parse_requests_using_flags(args, *args.packages)

    try:
        env = spk.current_env()
    except spk.NoEnvironmentError:
        if args.save:
            _LOGGER.warning("no current environment to install to")
            return
        raise

    if args.save:
        spec, filename, stage = _flags.parse_stage_specifier(args.save)
        if stage == "source":
            raise NotImplementedError("'source' stage is not yet supported")
        elif stage == "build":
            for r in requests:
                o = spk.api.opt_from_request(r)
                build = spec.build
                build.upsert_opt(o)
                spec.build = build

        elif stage == "install":
            for r in requests:
                spec.install.upsert_requirement(r)
        else:
            print(
                f"{Fore.RED}Unknown stage '{stage}', should be one of: 'source', 'build', 'install'{Fore.RESET}",
                file=sys.stderr,
            )
            sys.exit(1)

        spk.api.save_spec_file(filename, spec)
        print(f"{Fore.YELLOW}Updated: {filename}{Fore.RESET}")
        try:
            spkrs.active_runtime()
        except RuntimeError:
            return

    for solved in env.items():
        solver.add_request(solved.request)
    for request in requests:
        solver.add_request(request)

    runtime = solver.run()
    try:
        packages = runtime.solution()
    except spk.SolverError as e:
        print(f"{Fore.RED}{e}{Fore.RESET}")
        if args.verbose:
            graph = runtime.graph()
            print(spk.io.format_solve_graph(graph, verbosity=args.verbose))
        if args.verbose == 0:
            print(
                f"{Fore.YELLOW}{Style.DIM}try '--verbose/-v' for more info{Style.RESET_ALL}",
                file=sys.stderr,
            )
        elif args.verbose < 2:
            print(
                f"{Fore.YELLOW}{Style.DIM}try '-vv' for even more info{Style.RESET_ALL}",
                file=sys.stderr,
            )
        sys.exit(1)

    if not packages:
        print(f"Nothing to do.")
        return

    print("The following packages will be modified:\n")
    requested = set(r.pkg.name for r in solver.get_initial_state().pkg_requests)
    primary, tertiary = [], []
    for req, spec, _ in packages.items():
        if spec.pkg.name in requested:
            primary.append(spec)
            continue
        try:
            env.get(req.pkg.name)
        except KeyError:
            tertiary.append(spec)

    print("  Requested:")
    for spec in primary:
        end = "\n"
        if spec.pkg.build is None:
            end = f" {Fore.MAGENTA}[build from source]{Fore.RESET}\n"
        print("    " + spk.io.format_ident(spec.pkg), end=end)
    if tertiary:
        print("\n  Dependencies:")
        for spec in tertiary:
            end = "\n"
            if spec.pkg.build is None:
                end = f" {Fore.MAGENTA}[build from source]{Fore.RESET}\n"
            print("    " + spk.io.format_ident(spec.pkg), end=end)

    print("")

    if args.yes:
        pass
    elif input("Do you want to continue? [y/N]: ").lower() not in ("y", "yes"):
        print("Installation cancelled")
        sys.exit(1)

    compiled_solution = spk.build_required_packages(packages)
    spk.setup_current_runtime(compiled_solution)
