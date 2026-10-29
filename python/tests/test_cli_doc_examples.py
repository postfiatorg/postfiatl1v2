"""The RPC commands and client endpoints in the Python and RPC docs parse as written."""

from __future__ import annotations

import argparse
import contextlib
import importlib.machinery
import importlib.util
import io
import re
import shlex
import sys
import unittest
from pathlib import Path
from typing import Callable

from postfiat_rpc import Endpoint, pft_token, pft_token_ui, private_path_ui
from postfiat_rpc.__main__ import build_parser, parse_endpoint_ref

REPO_ROOT = Path(__file__).resolve().parents[2]
FAKE_ADDRESS = "pf0000000000000000000000000000000000000000"
VALIDATOR_RPC_PORT = 27650
MODULE_COMMAND = "python -m postfiat_rpc"
ACCOUNT_TX_SCRIPT = "scripts/postfiat-rpc-account-tx"


def documented_commands(relative: str, command: str = MODULE_COMMAND) -> list[list[str]]:
    """Return the arguments of each `command` invocation in the doc's fenced blocks."""
    program = command.split()[-1]
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
        if re.search(re.escape(command) + r"(?=\s|$)", pending):
            argv = shlex.split(pending.replace("<address>", FAKE_ADDRESS))
            commands.append(argv[argv.index(program) + 1 :])
        pending = ""
    return commands


def load_script(relative: str):
    loader = importlib.machinery.SourceFileLoader(Path(relative).name.replace("-", "_"), str(REPO_ROOT / relative))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    module = importlib.util.module_from_spec(spec)
    sys.modules[loader.name] = module  # the script's dataclasses resolve through sys.modules
    loader.exec_module(module)
    return module


class CliDocExampleTests(unittest.TestCase):
    def parse(self, relative: str, parse_args: Callable[[list[str]], argparse.Namespace], argv: list[str]):
        stderr = io.StringIO()
        try:
            with contextlib.redirect_stderr(stderr):
                return parse_args(argv)
        except SystemExit:
            self.fail(f"{relative}: {stderr.getvalue().strip().splitlines()[-1]}")

    def test_documented_commands_parse_and_target_the_validator_rpc_port(self) -> None:
        for relative, minimum in (("docs/python/quickstart.md", 4), ("docs/rpc/examples.md", 3)):
            commands = documented_commands(relative)
            self.assertGreaterEqual(len(commands), minimum, relative)
            for argv in commands:
                with self.subTest(doc=relative, argv=argv):
                    args = self.parse(relative, build_parser().parse_args, argv)
                    self.assertEqual(parse_endpoint_ref(args.endpoint)[1].port, VALIDATOR_RPC_PORT)

    def test_documented_pft_token_commands_parse(self) -> None:
        relative = "docs/rpc/examples.md"
        commands = documented_commands(relative, "python -m postfiat_rpc.pft_token")
        self.assertEqual(len(commands), 2)
        for argv in commands:
            with self.subTest(argv=argv):
                args = self.parse(relative, pft_token.build_parser().parse_args, argv)
                self.assertTrue(args.record or (args.token and args.treasury))
        commands = documented_commands(relative, "python -m postfiat_rpc.pft_token_ui")
        self.assertEqual(len(commands), 1)
        args = self.parse(relative, pft_token_ui.build_parser().parse_args, commands[0])
        self.assertTrue(args.record)

    def test_documented_private_path_ui_command_parses(self) -> None:
        relative = "docs/rpc/examples.md"
        commands = documented_commands(relative, "python -m postfiat_rpc.private_path_ui")
        self.assertEqual(len(commands), 1)
        args = self.parse(relative, private_path_ui.build_parser().parse_args, commands[0])
        self.assertEqual(parse_endpoint_ref(args.endpoint)[1].port, VALIDATOR_RPC_PORT)

    def test_python_examples_target_the_validator_rpc_port(self) -> None:
        relative = "docs/python/examples.md"
        endpoints = re.findall(r'PostFiatRpcClient\("([^"]+)"', (REPO_ROOT / relative).read_text(encoding="utf-8"))
        self.assertGreaterEqual(len(endpoints), 4)
        for raw in endpoints:
            with self.subTest(client=raw):
                self.assertEqual(Endpoint.parse(raw).port, VALIDATOR_RPC_PORT)

        script = load_script(ACCOUNT_TX_SCRIPT)
        commands = documented_commands(relative, ACCOUNT_TX_SCRIPT)
        self.assertEqual(len(commands), 1)
        for argv in commands:
            with self.subTest(argv=argv):
                args = self.parse(relative, script.parse_args, argv)
                self.assertEqual(args.address, FAKE_ADDRESS)
                for index, raw in enumerate(args.endpoint):
                    self.assertEqual(script.parse_endpoint(raw, index).endpoint.port, VALIDATOR_RPC_PORT)


if __name__ == "__main__":
    unittest.main()
