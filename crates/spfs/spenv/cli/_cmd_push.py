import argparse

from colorama import Fore

import spenv


def register(sub_parsers: argparse._SubParsersAction) -> None:

    config = spenv.get_config()

    push_cmd = sub_parsers.add_parser("push", help=_push.__doc__)
    push_cmd.add_argument(
        "refs", metavar="REF", nargs="+", help="the references to push"
    )
    push_cmd.add_argument(
        "--remote",
        default="origin",
        help=f"the name of the remote server to push to, one of: {', '.join(config.list_remote_names())}",
    )
    push_cmd.set_defaults(func=_push)


def _push(args: argparse.Namespace) -> None:
    """Push one or more objects to a remote repository."""

    for ref in args.refs:
        spenv.push_ref(ref, args.remote)
