from __future__ import annotations

import importlib.util
from pathlib import Path
import types

import pytest


ROOT = Path(__file__).resolve().parents[2]
WATCHER_PATH = Path(__file__).with_name("fire_watcher.py")
A666 = Path("/home/postfiat/repos/a666-eth-fast-lane-combined-20260724")


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


@pytest.fixture()
def watcher(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    module = load_module("a666_leg3b_fire_watcher_test", WATCHER_PATH)
    monkeypatch.setattr(module, "BASE", tmp_path)
    monkeypatch.setattr(module, "FIRE", tmp_path / "fire")
    monkeypatch.setattr(module, "FUND_INTENT", tmp_path / "fire/leg3b0-intent.json")
    monkeypatch.setattr(module, "FUND_REPORT", tmp_path / "fire/leg3b0-report.json")
    return module


def base_intent(watcher, phase: str) -> dict:
    return {
        "schema": "postfiat.a666.leg3b0.intent.v1",
        "phase": phase,
        "owner": watcher.OWNER,
        "signer": watcher.SIGNER,
        "amount_wei": watcher.FUND_WEI,
        "chain_id": 1,
        "label": "leg3b0-signer-funding",
    }


def test_external_trigger_is_exact_authorized_001_eth(watcher) -> None:
    assert watcher.REQUIRED_SIGNER_WEI == watcher.FUND_WEI == 10**16


def test_deadline_guard_fails_closed(watcher, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        watcher.time,
        "time",
        lambda: watcher.DEADLINE - watcher.DEADLINE_MARGIN_S,
    )
    with pytest.raises(SystemExit) as error:
        watcher.deadline_guard("test mutation")
    assert error.value.code == 2
    assert "STOP-no-retry" in (watcher.BASE / "STOP.txt").read_text()


def test_started_funding_attempt_reconciles_without_resend(
    watcher, monkeypatch: pytest.MonkeyPatch
) -> None:
    intent = base_intent(watcher, "broadcast_attempt_started")
    evidence = {"tx_hash": "0x" + "11" * 32, "status": 1}
    monkeypatch.setattr(watcher, "load_funding_intent", lambda: intent)
    monkeypatch.setattr(
        watcher,
        "recover_funding_from_report_or_journal",
        lambda current, wait_seconds: evidence,
    )
    monkeypatch.setattr(
        watcher,
        "persist_verified_funding",
        lambda current, result, recovered: watcher.REQUIRED_SIGNER_WEI,
    )
    monkeypatch.setattr(
        watcher,
        "verifier_checkpoint",
        lambda: (691, watcher.PRIOR_COMMITMENT),
    )
    monkeypatch.setattr(
        watcher,
        "run_step",
        lambda *args, **kwargs: pytest.fail("a reconciled attempt must never resend"),
    )
    assert watcher.reconcile_or_fund_signer() == watcher.REQUIRED_SIGNER_WEI


def test_fresh_balance_recheck_skips_funding_broadcast(
    watcher, monkeypatch: pytest.MonkeyPatch
) -> None:
    intent = base_intent(watcher, "prepared")
    writes: list[dict] = []
    monkeypatch.setattr(watcher, "load_funding_intent", lambda: intent)
    monkeypatch.setattr(
        watcher, "signer_balance_wei", lambda: watcher.REQUIRED_SIGNER_WEI
    )
    monkeypatch.setattr(
        watcher,
        "atomic_write_json",
        lambda path, value: writes.append(dict(value)),
    )
    monkeypatch.setattr(
        watcher,
        "run_step",
        lambda *args, **kwargs: pytest.fail("external funding must skip 3b0"),
    )
    assert watcher.reconcile_or_fund_signer() == watcher.REQUIRED_SIGNER_WEI
    assert writes[-1]["phase"] == "skipped_external_funding"


def test_partial_external_funding_refuses_additional_001(
    watcher, monkeypatch: pytest.MonkeyPatch
) -> None:
    intent = base_intent(watcher, "prepared")
    monkeypatch.setattr(watcher, "load_funding_intent", lambda: intent)
    monkeypatch.setattr(watcher, "signer_balance_wei", lambda: 5 * 10**15)
    with pytest.raises(SystemExit) as error:
        watcher.reconcile_or_fund_signer()
    assert error.value.code == 2


def test_checkpoint_skip_requires_exact_target_commitment(
    watcher, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(watcher, "deadline_guard", lambda step: None)
    monkeypatch.setattr(
        watcher, "signer_balance_wei", lambda: watcher.REQUIRED_SIGNER_WEI
    )
    monkeypatch.setattr(
        watcher,
        "verifier_checkpoint",
        lambda: (756, "0x" + "00" * 32),
    )
    monkeypatch.setattr(
        watcher,
        "run_step",
        lambda *args, **kwargs: pytest.fail("bad checkpoint must stop before a step"),
    )
    with pytest.raises(SystemExit) as error:
        watcher.fire_sequence(need_funding=False)
    assert error.value.code == 2


def test_run_step_checks_deadline_after_persisting_command(
    watcher, monkeypatch: pytest.MonkeyPatch
) -> None:
    events: list[str] = []
    monkeypatch.setattr(
        watcher,
        "atomic_write_json",
        lambda path, value: events.append(f"write:{value['phase']}"),
    )
    monkeypatch.setattr(
        watcher,
        "deadline_guard",
        lambda step: events.append(f"deadline:{step}"),
    )
    monkeypatch.setattr(
        watcher.subprocess,
        "run",
        lambda *args, **kwargs: (
            events.append("subprocess")
            or types.SimpleNamespace(returncode=0, stdout="", stderr="")
        ),
    )
    watcher.run_step("unit", ["true"], mutation=True)
    assert events[:3] == ["write:prepared", "deadline:unit", "subprocess"]


@pytest.mark.parametrize(
    ("name", "relative", "helper", "message"),
    [
        (
            "accept_mint_deadline_test",
            "scripts/a666-mainnet-accept-and-mint.py",
            "enforce_mutation_deadline",
            "accept proof",
        ),
        (
            "checkpoint_deadline_test",
            "scripts/a666-mainnet-advance-pftl-checkpoint.py",
            "enforce_mutation_deadline",
            None,
        ),
    ],
)
def test_underlying_submit_helpers_enforce_not_after_epoch(
    name: str,
    relative: str,
    helper: str,
    message: str | None,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    module = load_module(name, A666 / relative)
    monkeypatch.setattr(module, "MUTATION_NOT_AFTER_EPOCH", 100)
    monkeypatch.setattr(module.time, "time", lambda: 100)
    with pytest.raises(RuntimeError, match="deadline"):
        function = getattr(module, helper)
        if message is None:
            function()
        else:
            function(message)
