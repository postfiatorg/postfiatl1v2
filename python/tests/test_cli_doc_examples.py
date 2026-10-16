"""The `python -m postfiat_rpc` commands in the Python and RPC docs parse as written."""

from __future__ import annotations

import contextlib
import io
import shlex
import unittest
from pathlib import Path

from postfiat_rpc.__main__ import build_parser, parse_endpoint_ref

REPO_ROOT = Path(__file__).resolve().parents[2]
FAKE_ADDRESS = "pf0000000000000000000000000000000000000000"
VALIDATOR_RPC_PORT = 27650


def documented_commands(relative: str) -> list[list[str]]:
    """Return the argv of each `python -m postfiat_rpc` command in the doc's fenced blocks."""
    commands: list[list[str]] = []
    in_fence = False
    pending = ""
    for line in (REPO_ROOT / relative).read_text(encoding="utf-8").splitlines():
        if line.startswith("```"):
            in_fence = not in_fence
            continue
        if not in_fence:
            continue
        pending += line.rstrip()
        if pending.endswith("\\"):
            pending = pending[:-1] + " "
            continue
        if "python -m postfiat_rpc" in pending:
            argv = shlex.split(pending.replace("<address>", FAKE_ADDRESS))
            commands.append(argv[argv.index("postfiat_rpc") + 1 :])
        pending = ""
    return commands


class CliDocExampleTests(unittest.TestCase):
    def test_documented_commands_parse_and_target_the_validator_rpc_port(self) -> None:
        for relative, minimum in (("docs/python/quickstart.md", 4), ("docs/rpc/examples.md", 3)):
            commands = documented_commands(relative)
            self.assertGreaterEqual(len(commands), minimum, relative)
            for argv in commands:
                with self.subTest(doc=relative, argv=argv):
                    stderr = io.StringIO()
                    try:
                        with contextlib.redirect_stderr(stderr):
                            args = build_parser().parse_args(argv)
                    except SystemExit:
                        self.fail(f"{relative}: {stderr.getvalue().strip().splitlines()[-1]}")
                    self.assertEqual(parse_endpoint_ref(args.endpoint)[1].port, VALIDATOR_RPC_PORT)


if __name__ == "__main__":
    unittest.main()
