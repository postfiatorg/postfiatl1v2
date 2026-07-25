from __future__ import annotations

import json
from pathlib import Path
import stat
import subprocess
import tempfile
import threading
import time
import unittest

from ...coordinator.signing import Ed25519Signer
from ..composition import (
    ARMED_PROCESS_ACK,
    CompositionError,
    GraduationRuntimeFacade,
    SecureStatePaths,
    SOURCE_RELEASE_SCHEMA,
    SOURCE_RELEASE_TARGETS,
    exposure_limits,
    load_pinned_lnd_proto_modules,
    load_quote_signer,
    load_strict_json,
    prepare_secure_state,
    validate_armed_source_release,
    validate_aggregate_onramp_capacity,
    validate_activation,
    validate_receive_only_macaroon_path,
)
from ..lnd_connection import MainnetLndConnection
from ..operator_control import (
    OperatorControl,
    OperatorControlError,
    OperatorControlServer,
    send_authorization,
)
from ..policy import ExecutionMode, RealValuePolicy
from ..cli import _RecoveryWorker
from .common import policy_mapping


class CompositionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve() / "state"

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_prepare_creates_only_private_secret_material(self) -> None:
        first = prepare_secure_state(self.root)
        second = prepare_secure_state(self.root)
        self.assertEqual(
            first["quote_signer_public_key_hex"],
            second["quote_signer_public_key_hex"],
        )
        self.assertNotIn("seed", first)
        self.assertNotIn("api_session_token", first)
        paths = SecureStatePaths.under(self.root)
        for directory in (
            paths.root,
            paths.config_dir,
            paths.secrets_dir,
            paths.database_dir,
            paths.control_dir,
            paths.artifact_dir,
        ):
            self.assertEqual(stat.S_IMODE(directory.stat().st_mode), 0o700)
        self.assertEqual(stat.S_IMODE(paths.quote_seed.stat().st_mode), 0o600)
        self.assertEqual(
            stat.S_IMODE(paths.api_session_token.stat().st_mode),
            0o600,
        )
        self.assertEqual(paths.quote_seed.stat().st_size, 32)
        self.assertEqual(paths.api_session_token.stat().st_size, 32)

    def test_state_root_rejects_symlink_traversal_and_broad_path(self) -> None:
        real = Path(self.temporary.name) / "real"
        real.mkdir()
        link = Path(self.temporary.name) / "link"
        link.symlink_to(real, target_is_directory=True)
        with self.assertRaisesRegex(CompositionError, "symbolic link"):
            SecureStatePaths.under(link / "state")
        with self.assertRaisesRegex(CompositionError, "broad"):
            SecureStatePaths.under(Path("/home/postfiat"))

    def test_composition_requires_canonical_receive_only_macaroon_path(
        self,
    ) -> None:
        paths = SecureStatePaths.under(self.root)

        def connection(path: Path) -> MainnetLndConnection:
            return MainnetLndConnection.from_mapping(
                {
                    "schema": "postfiat.lightning_mainnet_lnd_connection.v1",
                    "endpoint": "127.0.0.1:11009",
                    "tls_server_name": "localhost",
                    "tls_cert_path": str(self.root / "lnd/tls.cert"),
                    "tls_cert_sha256": "11" * 32,
                    "macaroon_path": str(path),
                    "macaroon_sha256": "22" * 32,
                    "macaroon_profile": (
                        "LIGHTNING_NAVCOIN_RECEIVE_ONLY_V1"
                    ),
                    "ready_timeout_seconds": 10,
                }
            )

        self.assertEqual(
            validate_receive_only_macaroon_path(
                paths, connection(paths.receive_only_macaroon)
            ),
            paths.receive_only_macaroon,
        )
        with self.assertRaisesRegex(CompositionError, "canonical.*receive-only"):
            validate_receive_only_macaroon_path(
                paths,
                connection(
                    self.root
                    / "lnd/data/chain/bitcoin/mainnet/admin.macaroon"
                ),
            )

    def test_strict_json_rejects_duplicates_and_group_write(self) -> None:
        duplicate = Path(self.temporary.name) / "duplicate.json"
        duplicate.write_text('{"a":1,"a":2}', encoding="ascii")
        duplicate.chmod(0o600)
        with self.assertRaisesRegex(CompositionError, "duplicate"):
            load_strict_json(duplicate, "test config")
        config = Path(self.temporary.name) / "config.json"
        config.write_text('{"a":1}', encoding="ascii")
        config.chmod(0o620)
        with self.assertRaisesRegex(CompositionError, "group/world writable"):
            load_strict_json(config, "test config")

    def test_quote_seed_must_match_policy_public_key(self) -> None:
        seed = bytes.fromhex("19" * 32)
        seed_path = Path(self.temporary.name) / "quote.seed"
        seed_path.write_bytes(seed)
        seed_path.chmod(0o600)
        signer = Ed25519Signer.from_private_bytes(seed)
        mapping = policy_mapping()
        mapping["quote_signer_public_key_hex"] = signer.public_key_bytes().hex()
        policy = RealValuePolicy.from_mapping(mapping)
        loaded = load_quote_signer(seed_path, policy)
        self.assertEqual(loaded.public_key_bytes(), signer.public_key_bytes())
        mapping["quote_signer_public_key_hex"] = "99" * 32
        mismatched = RealValuePolicy.from_mapping(mapping)
        with self.assertRaisesRegex(CompositionError, "does not match"):
            load_quote_signer(seed_path, mismatched)

    def test_mode_activation_is_asymmetric_and_explicit(self) -> None:
        dry = RealValuePolicy.from_mapping(policy_mapping(mode="DRY_RUN"))
        validate_activation(dry, armed_ack=None, signer_key_file=None)
        with self.assertRaisesRegex(CompositionError, "DRY_RUN refuses"):
            validate_activation(
                dry,
                armed_ack=ARMED_PROCESS_ACK,
                signer_key_file="/tmp/not-opened",
            )
        armed = RealValuePolicy.from_mapping(policy_mapping(mode="ARMED"))
        with self.assertRaisesRegex(CompositionError, "acknowledgement"):
            validate_activation(armed, armed_ack=None, signer_key_file="/tmp/key")
        with self.assertRaisesRegex(CompositionError, "signer"):
            validate_activation(armed, armed_ack=ARMED_PROCESS_ACK, signer_key_file=None)
        validate_activation(
            armed,
            armed_ack=ARMED_PROCESS_ACK,
            signer_key_file="/tmp/key",
        )

    def test_exposure_caps_use_nav_and_asset_precision(self) -> None:
        route = RealValuePolicy.from_mapping(policy_mapping())
        nav_per_unit = 1_035_074_022
        limits = exposure_limits(
            route,
            nav_per_unit_usd_e8=nav_per_unit,
        )
        expected_per = (
            route.max_per_run_usd_e8 * 10**route.pftl_asset_precision
            + nav_per_unit
            - 1
        ) // nav_per_unit
        expected_aggregate = (
            route.max_lifetime_usd_e8 * 10**route.pftl_asset_precision
            + nav_per_unit
            - 1
        ) // nav_per_unit
        self.assertEqual(limits.per_principal_atoms, expected_per)
        self.assertEqual(limits.aggregate_atoms, expected_aggregate)
        self.assertLessEqual(limits.per_principal_atoms, limits.aggregate_atoms)
        validate_aggregate_onramp_capacity(
            limits,
            coordinator_inventory_atoms=expected_aggregate,
            user_receive_headroom_atoms=expected_aggregate,
        )
        with self.assertRaisesRegex(CompositionError, "inventory"):
            validate_aggregate_onramp_capacity(
                limits,
                coordinator_inventory_atoms=expected_aggregate - 1,
                user_receive_headroom_atoms=expected_aggregate,
            )
        with self.assertRaisesRegex(CompositionError, "headroom"):
            validate_aggregate_onramp_capacity(
                limits,
                coordinator_inventory_atoms=expected_aggregate,
                user_receive_headroom_atoms=expected_aggregate - 1,
            )

    def test_generated_proto_digest_mismatch_fails_before_import(self) -> None:
        proto = Path(self.temporary.name) / "proto"
        proto.mkdir(mode=0o700)
        for name in (
            "lightning_pb2.py",
            "lightning_pb2_grpc.py",
            "router_pb2.py",
            "router_pb2_grpc.py",
        ):
            path = proto / name
            path.write_text("# wrong\n", encoding="ascii")
            path.chmod(0o600)
        with self.assertRaisesRegex(CompositionError, "digest mismatch"):
            load_pinned_lnd_proto_modules(proto)

    def test_armed_source_release_requires_exact_clean_commit_and_tree(self) -> None:
        repository = Path(self.temporary.name) / "release-repo"
        repository.mkdir()

        def git(*arguments: str) -> str:
            completed = subprocess.run(
                ["/usr/bin/git", "-C", str(repository), *arguments],
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
            return completed.stdout.strip()

        git("init", "--quiet")
        tracked = repository / "tools/lightning_navcoin_demo/release.py"
        tracked.parent.mkdir(parents=True)
        tracked.write_text("RELEASE = 1\n", encoding="ascii")
        git("add", "tools/lightning_navcoin_demo/release.py")
        git(
            "-c",
            "user.name=PostFiat Test",
            "-c",
            "user.email=test@postfiat.invalid",
            "commit",
            "--quiet",
            "-m",
            "release",
        )
        pin = Path(self.temporary.name) / "source-release.json"
        pin.write_text(
            json.dumps(
                {
                    "schema": SOURCE_RELEASE_SCHEMA,
                    "git_commit": git("rev-parse", "--verify", "HEAD^{commit}"),
                    "git_tree": git("rev-parse", "--verify", "HEAD^{tree}"),
                    "clean": True,
                    "targets": list(SOURCE_RELEASE_TARGETS),
                }
            ),
            encoding="ascii",
        )
        pin.chmod(0o600)
        report = validate_armed_source_release(pin, repo_root=repository)
        self.assertTrue(report["clean"])
        tracked.write_text("RELEASE = 2\n", encoding="ascii")
        with self.assertRaisesRegex(CompositionError, "dirty"):
            validate_armed_source_release(pin, repo_root=repository)


class _FakeRuntime:
    def __init__(self, mode: ExecutionMode) -> None:
        self.policy = type("Policy", (), {"mode": mode})()
        self.calls: list[tuple[str, object]] = []

    def authorize_swap(self, swap_id: str, envelope: object) -> dict[str, object]:
        self.calls.append((swap_id, envelope))
        return {
            "swap_id": swap_id,
            "state": "PFTL_LOCK_FINAL",
            "authorized": True,
        }


class _FakeGraduationRuntime:
    def __init__(self, *, direction: str, state: str) -> None:
        self.policy = type("Policy", (), {"mode": ExecutionMode.ARMED})()
        self._swap = {"direction": direction, "state": state}
        self.journal = type(
            "Journal",
            (),
            {"get_swap": lambda inner_self, _swap_id: dict(self._swap)},
        )()
        self.calls: list[str] = []

    def public_status(self) -> dict[str, object]:
        return {
            "status": "GREEN",
            "mode": "ARMED",
            "can_execute": True,
            "hold_reasons": [],
        }

    def public_swap(self, _swap_id: str) -> dict[str, object]:
        self.calls.append("public_swap")
        return {"state": self._swap["state"]}

    def create_quote(self, _request: object) -> dict[str, object]:
        self.calls.append("create_quote")
        return {"ok": True}

    def authorize_swap(self, _swap_id: str, _envelope: object) -> dict[str, object]:
        self.calls.append("authorize_swap")
        return {"ok": True}

    def execute_offramp(self, _swap_id: str) -> dict[str, object]:
        self.calls.append("execute_offramp")
        return {"ok": True}

    def recover_swap(self, _swap_id: str) -> dict[str, object]:
        self.calls.append("recover_swap")
        return {"ok": True}


class GraduationDirectionTests(unittest.TestCase):
    def test_new_reverse_quote_and_authorization_are_held(self) -> None:
        runtime = _FakeGraduationRuntime(
            direction="pftl_to_lightning",
            state="PFTL_LOCK_SUBMITTED",
        )
        facade = GraduationRuntimeFacade(runtime)  # type: ignore[arg-type]
        with self.assertRaisesRegex(CompositionError, "reverse"):
            facade.create_quote({"direction": "pftl_to_lightning"})
        with self.assertRaisesRegex(CompositionError, "reverse"):
            facade.authorize_swap("ab" * 32, {})
        self.assertEqual(runtime.calls, [])
        self.assertIn(
            "HOLD",
            facade.public_status()["direction_execution"]["pftl_to_lightning"],
        )

    def test_onramp_remains_enabled(self) -> None:
        runtime = _FakeGraduationRuntime(
            direction="lightning_to_pftl",
            state="PFTL_LOCK_SUBMITTED",
        )
        facade = GraduationRuntimeFacade(runtime)  # type: ignore[arg-type]
        self.assertTrue(
            facade.create_quote({"direction": "lightning_to_pftl"})["ok"]
        )
        self.assertTrue(facade.authorize_swap("ab" * 32, {})["ok"])
        self.assertEqual(runtime.calls, ["create_quote", "authorize_swap"])

    def test_reverse_recovery_only_after_durable_payment_attempt(self) -> None:
        runtime = _FakeGraduationRuntime(
            direction="pftl_to_lightning",
            state="PFTL_LOCK_FINAL",
        )
        facade = GraduationRuntimeFacade(runtime)  # type: ignore[arg-type]
        with self.assertRaisesRegex(CompositionError, "disabled"):
            facade.recover_swap("ab" * 32)
        runtime._swap["state"] = "LN_IN_FLIGHT"
        self.assertTrue(facade.recover_swap("ab" * 32)["ok"])
        self.assertEqual(runtime.calls, ["recover_swap"])

    def test_changed_release_blocks_new_value_and_invoice_reveal(self) -> None:
        runtime = _FakeGraduationRuntime(
            direction="lightning_to_pftl",
            state="PFTL_LOCK_FINAL",
        )
        checks: list[str] = []

        def changed_release() -> Mapping[str, object]:
            checks.append("checked")
            raise CompositionError("release changed")

        facade = GraduationRuntimeFacade(
            runtime,  # type: ignore[arg-type]
            release_guard=changed_release,
        )
        status = facade.public_status()
        self.assertEqual(status["mode"], "HOLD")
        self.assertFalse(status["can_execute"])
        self.assertIn(
            "coordinator_or_wallet_release_changed",
            status["hold_reasons"],
        )
        with self.assertRaisesRegex(CompositionError, "release changed"):
            facade.create_quote({"direction": "lightning_to_pftl"})
        with self.assertRaisesRegex(CompositionError, "release changed"):
            facade.authorize_swap("ab" * 32, {})
        with self.assertRaisesRegex(CompositionError, "release changed"):
            facade.public_swap("ab" * 32)

        runtime._swap["state"] = "PFTL_FINISH_FINAL"
        self.assertEqual(
            facade.public_swap("ab" * 32)["state"],
            "PFTL_FINISH_FINAL",
        )
        self.assertGreaterEqual(len(checks), 4)


class RecoveryShutdownTests(unittest.TestCase):
    def test_close_waits_for_inflight_reconciliation(self) -> None:
        entered = threading.Event()
        release = threading.Event()
        action = type("Action", (), {"swap_id": "ab" * 32})()

        class Runtime:
            service = type(
                "Service",
                (),
                {"recovery_plan": lambda _self: [action]},
            )()

            @staticmethod
            def recover_swap(_swap_id: str) -> None:
                entered.set()
                release.wait(5)

        worker = _RecoveryWorker(Runtime(), interval_seconds=1)
        worker.start()
        self.assertTrue(entered.wait(2))
        closer = threading.Thread(target=worker.close)
        closer.start()
        time.sleep(0.05)
        self.assertTrue(closer.is_alive())
        release.set()
        closer.join(2)
        self.assertFalse(closer.is_alive())


class OperatorControlTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        self.root.chmod(0o700)
        self.swap_id = "ab" * 32
        self.envelope = {
            "algorithm": "Ed25519",
            "public_key": "public-envelope-only",
            "authorization": {"swap_id": self.swap_id},
            "signature": "signed-elsewhere",
        }

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def test_dry_run_control_rejects_authorization(self) -> None:
        runtime = _FakeRuntime(ExecutionMode.DRY_RUN)
        control = OperatorControl(runtime)
        with self.assertRaisesRegex(OperatorControlError, "disabled"):
            control.dispatch(
                {
                    "schema": "postfiat.lightning_operator_control_request.v1",
                    "action": "authorize_swap",
                    "swap_id": self.swap_id,
                    "authorization": self.envelope,
                }
            )
        self.assertEqual(runtime.calls, [])

    def test_owner_only_socket_dispatches_public_signed_envelope(self) -> None:
        runtime = _FakeRuntime(ExecutionMode.ARMED)
        socket_path = self.root / "operator.sock"
        permit_path = self.root / "permit.json"
        permit_path.write_text(json.dumps(self.envelope), encoding="ascii")
        permit_path.chmod(0o600)
        with OperatorControlServer(socket_path, runtime):
            socket_mode = stat.S_IMODE(socket_path.lstat().st_mode)
            self.assertEqual(socket_mode, 0o600)
            result = send_authorization(
                socket_path=socket_path,
                swap_id=self.swap_id,
                authorization_path=permit_path,
            )
        self.assertFalse(socket_path.exists())
        self.assertEqual(result["swap_id"], self.swap_id)
        self.assertEqual(runtime.calls, [(self.swap_id, self.envelope)])

    def test_socket_rejects_unsafe_preexisting_path(self) -> None:
        path = self.root / "operator.sock"
        path.write_text("do not replace", encoding="ascii")
        runtime = _FakeRuntime(ExecutionMode.ARMED)
        with self.assertRaisesRegex(OperatorControlError, "unsafe"):
            OperatorControlServer(path, runtime)
        self.assertEqual(path.read_text(encoding="ascii"), "do not replace")

    def test_socket_close_waits_for_inflight_authorization(self) -> None:
        entered = threading.Event()
        release = threading.Event()

        class SlowRuntime(_FakeRuntime):
            def authorize_swap(
                self, swap_id: str, envelope: object
            ) -> dict[str, object]:
                entered.set()
                release.wait(5)
                return super().authorize_swap(swap_id, envelope)

        runtime = SlowRuntime(ExecutionMode.ARMED)
        socket_path = self.root / "slow.sock"
        permit_path = self.root / "permit.json"
        permit_path.write_text(json.dumps(self.envelope), encoding="ascii")
        permit_path.chmod(0o600)
        server = OperatorControlServer(socket_path, runtime)
        server.start()
        client_result: list[object] = []

        def send() -> None:
            client_result.append(
                send_authorization(
                    socket_path=socket_path,
                    swap_id=self.swap_id,
                    authorization_path=permit_path,
                )
            )

        client = threading.Thread(target=send)
        client.start()
        self.assertTrue(entered.wait(2))
        closer = threading.Thread(target=server.close)
        closer.start()
        time.sleep(0.05)
        self.assertTrue(closer.is_alive())
        release.set()
        closer.join(2)
        client.join(2)
        self.assertFalse(closer.is_alive())
        self.assertFalse(client.is_alive())
        self.assertEqual(client_result[0]["authorized"], True)


if __name__ == "__main__":
    unittest.main()
