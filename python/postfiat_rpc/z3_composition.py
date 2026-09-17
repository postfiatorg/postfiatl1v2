"""Explicit Z3 command composition. Default: stop; dry-run: print and skeleton.

Every invocation can execute at most one named step, including preparation.
A durable attempt marker is written before execution. A failure/interruption
cannot be retried by another confirmation. Progress comes from independently
retained readbacks, never subprocess exit status. Signer arguments are paths
only and are excluded from the public skeleton and attempt journal.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
from datetime import datetime, timezone
import shlex
import subprocess
import sys
from typing import Any, Callable

from . import z3_cycle as z3

RPC = "https://rpc.testnet.arc.network"
TOKEN = "0x3600000000000000000000000000000000000000"
REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts/z3-cycle-dry-run.py"
ARC_MARKER = "tools/pfusdc-tier4-prover/src/arc_ingress_capture.rs"


@dataclass(frozen=True)
class Step:
    name: str
    argv: list[str]
    kind: str = "prepare"
    operation: str | None = None
    request: str | None = None

    @property
    def command_sha256(self) -> str:
        return hashlib.sha256(json.dumps(self.argv, separators=(",", ":")).encode()).hexdigest()


def driver():
    spec = importlib.util.spec_from_file_location("z3_reserve_driver", REPO / "scripts/a666-pfusdc-reserve-demo.py")
    if spec is None or spec.loader is None:
        raise z3.CycleError("reserve driver unavailable")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def metadata_hash(config: dict) -> str:
    return hashlib.sha256(json.dumps(config, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def validate_inputs(config: dict) -> None:
    z3.public(config)
    z3.validate_metadata(config["manifest"], skeleton=True)
    ids = driver().validate_identities(config["reserve_identities"])
    for field in set(ids) & set(config["manifest"]["identities"]):
        z3.same(ids[field], config["manifest"]["identities"][field], f"reserve identity/{field}")
    z3.same(ids["nav_program_vkey"], config["manifest"]["proof_keys"]["nav"], "NAV proof key")
    z3.same(ids["nav_valuation_policy_hash"], config["manifest"]["policy_hashes"]["nav_valuation"],
            "NAV valuation policy")
    params = config["parameters"]
    amount = config["manifest"]["amount_atoms"]
    z3.require(amount <= z3.uint(params["cap_atoms"], "operator cap", positive=True), "amount exceeds cap")
    z3.require(z3.utc(params["window_start"]) <= z3.utc(config["manifest"]["utc_start"])
               <= z3.utc(params["window_end"]), "cycle outside operator window")
    for field in ("mint_amount_atoms", "reservation_ttl_blocks", "latency_bound_seconds"):
        z3.uint(params[field], field, positive=True)
    z3.require(re.fullmatch(r"0x[0-9a-fA-F]{64}", params["route_binding"]) is not None,
               "full EVM route binding required")
    z3.require(re.fullmatch(r"0x[0-9a-fA-F]{64}", params["deposit_nonce"]) is not None,
               "full deposit nonce required")
    z3.require(z3.ADDRESS.fullmatch(params["reservation_recipient"]) is not None,
               "full reservation recipient required")
    # The existing NAV builder is specifically A666. Reject incompatible inputs
    # before emitting a command that would silently inherit another authority.
    z3.same(config["manifest"]["accounts"]["route_operator"],
            "pffcb93d9f87a843a8aa34e1adf241f5d58143e81b", "A666 NAV issuer")
    z3.same(config["manifest"]["accounts"]["reserve_operator"],
            "pfd0c86d9084915e1fefd22eab891806397d5a5937", "A666 reserve operator")
    for name in ("node_bin", "prover_bin", "egress_elf", "data_dir", "work_dir",
                 "packet_dir", "remote_runner", "proposer_hosts_file",
                 "remote_binary", "remote_topology", "opening_nav_manifest",
                 "fresh_packet_operation"):
        value = config["paths"][name]
        z3.require(isinstance(value, str) and bool(value), f"missing explicit path/{name}")
    work, packet = (Path(config["paths"][key]).resolve() for key in ("work_dir", "packet_dir"))
    z3.require(not work.is_relative_to(packet) and not packet.is_relative_to(work),
               "signer-local work and public packet directories must be separate")
    z3.same(params["egress_release"], "arc-v2", "qualified Arc egress release")
    z3.same(config["manifest"]["proof_keys"]["egress"],
            "0x0036cbe7d36bbfe1118a3c544eeba74f3791d2a19bf5ec59b972f72d36416852",
            "arc-v2 egress key; an older pair requires its matching qualified tooling")
    z3.same(config["manifest"]["proof_keys"]["ingress"],
            "0x0050a8b0daed2fa75f44d4102b42204c34668a03d311cb727cc6ca3f8df5cf16",
            "candidate Arc ingress key")
    for field in ("reserve_operator", "bridge_settler"):
        z3.require(z3.ACCOUNT.fullmatch(config["manifest"]["accounts"][field]) is not None,
                   "full maintenance account required")



def compile_steps(config: dict, signers: dict, values: dict | None = None) -> list[Step]:
    validate_inputs(config)
    values = values or {}
    m, p, paths = config["manifest"], config["parameters"], config["paths"]
    ids, accounts = m["identities"], m["accounts"]
    work, packet = Path(paths["work_dir"]), Path(paths["packet_dir"])
    node, prover = paths["node_bin"], paths["prover_bin"]
    def w(name): return str(work / name)
    def v(name): return str(values.get(name, "@" + name + "@"))
    def py(name): return [sys.executable, str(REPO / "scripts" / name)]
    helper = [sys.executable, str(SCRIPT)]
    steps: list[Step] = []

    def add(name, argv, kind="prepare", operation=None, request=None):
        steps.append(Step(name, [str(arg) for arg in argv], kind, operation, request))

    def submit(name, request, operation):
        add(name, py("a666-ce22-remote-finality-op.py") + [
            "--ops-file", request, "--artifact-dir", w("finality/" + name),
            "--node-bin", node, "--remote-runner", paths["remote_runner"],
            "--proposer-hosts-file", paths["proposer_hosts_file"],
            "--remote-binary", paths["remote_binary"], "--remote-topology", paths["remote_topology"],
            "--timeout-seconds", str(p["latency_bound_seconds"])], "pftl", operation, request)

    def wrap(name, operation_path, account, signer, output):
        add(name, helper + ["wrap-operation", "--operation", operation_path, "--source", account,
                            "--signer-path", signer, "--output", output])

    add("preflight", helper + ["check-preflight", "--inputs", v("INPUTS"),
                               "--checkpoint", v("CHECKPOINT")])
    # Preflight requires sufficient existing allowance; an approval, if needed,
    # is a separately authorized pre-cycle transaction followed by a new baseline.
    add("arc-deposit", ["cast", "send", ids["source_vault_address"],
        "depositV2(uint256,string,bytes32,bytes32)", m["amount_atoms"], accounts["owner"],
        p["deposit_nonce"], p["route_binding"], "--rpc-url", RPC, "--chain", "5042002",
        "--from", accounts["arc_wallet"], "--keystore", signers["keystore"],
        "--password-file", signers["password_file"], "--json"], "arc")
    add("ingress-capture", [prover, "arc-ingress-capture", "--rpc", RPC,
        "--deposit-tx", v("DEPOSIT_TX"), "--route-id", p["route_binding"],
        "--vault", ids["source_vault_address"], "--token", TOKEN, "--output", w("ingress-witness.json")])
    add("ingress-proof", [prover, "arc-ingress", "--witness", w("ingress-witness.json"),
                          "--output-dir", w("ingress-proof"), "--prove"])
    add("ingress-bundle", [node, "vault-bridge-deposit-relay-bundle",
        "--receipt-file", w("arc-deposit.stdout.json"), "--vault-address", ids["source_vault_address"],
        "--token-address", TOKEN, "--asset-id", ids["settlement_asset_id"],
        "--policy-hash", ids["source_profile_hash"], "--route-epoch", ids["source_route_epoch"],
        "--proposer", accounts["proposer"], "--finalizer", accounts["finalizer"],
        "--claimer", accounts["owner"], "--expires-at-height", v("INGRESS_EXPIRES_HEIGHT"),
        "--observer-confirmation-depth", v("ARC_CONFIRMATION_DEPTH"),
        "--source-proof-kind", "sp1-arc-finality-v1",
        "--source-proof-file", w("ingress-proof/proof-calldata.bin"),
        "--source-public-values-file", w("ingress-proof/public-values.bin"), "--bundle", w("relay")])
    for role, account, signer in (
        ("propose", accounts["proposer"], signers["proposer"]),
        ("finalize", accounts["finalizer"], signers["finalizer"]),
        ("claim", accounts["owner"], signers["holder"]),
    ):
        output = w(role + ".ops.json")
        wrap("prepare-" + role, w("relay/" + role + ".operation.json"), account, signer, output)
        submit("ingress-" + role, output, "vault_bridge_deposit_" + role)
    add("prepare-subscription", py("a666-pfusdc-reserve-demo.py") + [
        "build-issue", "--identities", w("reserve-identities.json"),
        "--route-status", w("before-issue.route.json"), "--nav-manifest", paths["opening_nav_manifest"],
        "--subscriber", accounts["owner"], "--ethereum-recipient", p["reservation_recipient"],
        "--holder-key-file", signers["holder"], "--mint-amount-atoms", p["mint_amount_atoms"],
        "--current-height", v("ISSUE_HEIGHT"), "--reservation-ttl-blocks", p["reservation_ttl_blocks"],
        "--output-dir", w("issue")])
    for name, file, op in (
        ("reserve", "01-reserve.ops.json", "pftl_uniswap_order_reserve"),
        ("subscribe", "02-subscribe.ops.json", "pftl_uniswap_primary_subscribe_v2"),
        ("entitlement-release", "03-release-entitlement.ops.json", "pftl_uniswap_order_release"),
    ):
        submit(name, w("issue/" + file), op)
    verify_issue = py("a666-pfusdc-reserve-demo.py") + ["verify-issue",
        "--issue-manifest", w("issue/issue-manifest.json"), "--output", w("issue-verify.json")]
    for flag, prefix in (("before", "before-issue"), ("after-subscribe", "after-subscribe"),
                         ("after-release", "after-release")):
        for asset in ("route", "pfusdc", "a666"):
            verify_issue += ["--" + flag + "-" + asset, w(prefix + "." + asset + ".json")]
    add("verify-subscription", verify_issue)
    add("prepare-nav", py("a666-build-live-nav-mark-ops.py") + [
        "--packet-operation", paths["fresh_packet_operation"],
        "--pftl-status", w("after-release.pftl-status.json"), "--issuer-key-file", signers["issuer"],
        "--reserve-key-file", signers["reserve"], "--output-dir", w("nav")])
    submit("nav-submit", w("nav/01-reserve-submit.ops.json"), "nav_reserve_submit")
    submit("nav-finalize", w("nav/02-epoch-finalize.ops.json"), "nav_epoch_finalize")
    for paused in (True, False):
        name = "pause" if paused else "resume"
        add("prepare-route-" + name, helper + ["route-switch", "--route", ids["route_id"],
            "--operator", accounts["route_operator"], "--paused", str(paused).lower(),
            "--signer-path", signers["issuer"], "--output", w("route-" + name + ".ops.json")])
        if paused:
            submit("route-pause", w("route-pause.ops.json"), "pftl_uniswap_route_pause")
            add("prepare-route-epoch", py("a666-build-route-epoch-advance.py") + [
                "--route-status", w("paused.route.json"), "--nav-manifest", w("nav/live-nav-mark-manifest.json"),
                "--identities", w("reserve-identities.json"), "--operator", accounts["route_operator"],
                "--issuer-key-file", signers["issuer"], "--valid-from-height", v("ROUTE_HEIGHT"),
                "--output-dir", w("route-epoch")])
            submit("route-advance", w("route-epoch/route-epoch-advance.ops.json"),
                   "pftl_uniswap_route_epoch_advance")
        else:
            submit("route-resume", w("route-resume.ops.json"), "pftl_uniswap_route_pause")
    add("prepare-redemption", py("a666-pfusdc-reserve-demo.py") + [
        "build-redeem", "--identities", w("reserve-identities.json"),
        "--route-status", w("before-redeem.route.json"), "--nav-manifest", w("nav/live-nav-mark-manifest.json"),
        "--issue-manifest", w("issue/issue-manifest.json"), "--holder-key-file", signers["holder"],
        "--owner", accounts["owner"], "--current-height", v("REDEEM_HEIGHT"),
        "--output-dir", w("redeem")])
    submit("redeem", w("redeem/primary-redeem.ops.json"), "pftl_uniswap_primary_redeem")
    verify_redeem = py("a666-pfusdc-reserve-demo.py") + [
        "verify-redeem", "--redeem-manifest", w("redeem/redeem-manifest.json"),
        "--output", w("redeem-verify.json")]
    for flag, prefix in (("before", "before-redeem"), ("after", "after-redeem")):
        for asset in ("route", "pfusdc", "a666"):
            verify_redeem += ["--" + flag + "-" + asset, w(prefix + "." + asset + ".json")]
    add("verify-redemption", verify_redeem)
    add("burn-bundle", [node, "vault-bridge-burn-to-redeem-bundle", "--data-dir", paths["data_dir"],
        "--owner", accounts["owner"], "--asset-id", ids["settlement_asset_id"],
        "--bucket-id", ids["source_bucket_id"], "--amount-atoms", v("REDEEM_OUTPUT_ATOMS"),
        "--destination-ref", "evm-erc20:5042002:" + accounts["arc_wallet"].lower(), "--bundle", w("burn")])
    wrap("prepare-burn", w("burn/burn-to-redeem.operation.json"), accounts["owner"],
         signers["holder"], w("burn.ops.json"))
    submit("burn", w("burn.ops.json"), "vault_bridge_burn_to_redeem")
    add("egress-witness", [node, "pfusdc-egress-witness", "--data-dir", paths["data_dir"],
                           "--withdrawal-id", v("WITHDRAWAL_ID"), "--prior-checkpoint", v("PRIOR_CHECKPOINT")])
    add("egress-proof", [prover, "egress", "--egress-release", p["egress_release"],
        "--elf", paths["egress_elf"], "--witness", w("egress-witness.stdout.json"),
        "--output-dir", w("egress-proof"), "--prove"])
    add("arc-release", ["cast", "send", ids["source_vault_address"],
        "withdrawWithProof(bytes,bytes)", v("EGRESS_PUBLIC_VALUES_HEX"), v("EGRESS_PROOF_HEX"),
        "--rpc-url", RPC, "--chain", "5042002", "--from", accounts["arc_wallet"],
        "--keystore", signers["keystore"], "--password-file", signers["password_file"], "--json"], "arc")
    add("release-replay-check", ["cast", "call", ids["source_vault_address"],
        "withdrawWithProof(bytes,bytes)", v("EGRESS_PUBLIC_VALUES_HEX"), v("EGRESS_PROOF_HEX"),
        "--rpc-url", RPC, "--from", accounts["arc_wallet"]], "replay_check")
    wrap("prepare-egress-settlement", v("SETTLEMENT_OPERATION"), accounts["bridge_settler"],
         signers["settler"], w("settle.ops.json"))
    submit("egress-settle", w("settle.ops.json"), "vault_bridge_redeem_settle")
    add("seal-packet", [sys.executable, "-m", "postfiat_rpc.z3_cycle", "build",
                        "--layout", str(packet / "layout.json"), "--output", str(packet / "cycle.json")])
    add("final-convergence", [sys.executable, "-m", "postfiat_rpc.z3_cycle",
                              "verify", str(packet / "cycle.json")])
    return steps


def check_checkpoint(config: dict, checkpoint: dict, steps: list[Step], index: int) -> None:
    """Check the frozen, public gate evidence before a single command."""
    z3.same(checkpoint["inputs_sha256"], metadata_hash(config), "checkpoint input binding")
    z3.same(checkpoint["ready_for"], steps[index].name, "checkpoint next step")
    z3.same(checkpoint["identities"], config["manifest"]["identities"], "checkpoint route/asset")
    z3.same(checkpoint["accounts"], config["manifest"]["accounts"], "checkpoint accounts")
    stage = steps[index].name
    pending_egress = stage in {"egress-witness", "egress-proof", "arc-release",
                              "release-replay-check", "prepare-egress-settlement", "egress-settle"}
    z3.convergence(checkpoint["validators"], checkpoint,
                   {"mempool": 0, "reservations": int(stage == "subscribe"),
                    "egress": int(pending_egress)})
    height = checkpoint["finalized"]["height"]
    now = datetime.now(timezone.utc)
    captured, expires = z3.utc(checkpoint["captured_utc"]), z3.utc(checkpoint["expires_utc"])
    z3.require(captured <= now <= expires and (expires - captured).total_seconds() <= 300,
               "stale checkpoint; fresh readbacks required")
    z3.require(checkpoint["proof_valid_from"] <= height <= checkpoint["proof_valid_until"], "stale proof")
    for field in ("contract_keys_match", "source_enabled", "nonce_unused", "balances_confirmed",
                  "signing_recoverable", "quote_current", "allowance_sufficient"):
        z3.same(checkpoint[field], True, f"preflight/{field}")
        z3.require(type(checkpoint[field]) is bool, f"preflight boolean/{field}")
    # Entitlement is expected only while executing/reconciling the release.
    expected = config["parameters"]["mint_amount_atoms"] if steps[index].name == "entitlement-release" else 0
    z3.same(checkpoint["entitlement_atoms"], expected, "active entitlement")
    required = z3.uint(checkpoint["required_capacity_atoms"], "required capacity", positive=True)
    available = z3.uint(checkpoint["available_capacity_atoms"], "available capacity")
    z3.require(required <= available, "insufficient capacity")
    if stage == "arc-deposit":
        amount = config["manifest"]["amount_atoms"]
        z3.require(z3.uint(checkpoint["arc_usdc_balance_atoms"], "Arc balance") >= amount,
                   "insufficient confirmed Arc USDC")
        z3.require(z3.uint(checkpoint["allowance_atoms"], "allowance") >= amount,
                   "insufficient vault allowance")
        gas = z3.uint(checkpoint["arc_required_gas_wei"], "gas budget", positive=True)
        z3.require(z3.uint(checkpoint["arc_wallet_wei"], "Arc native balance")
                   >= amount * 10**12 + gas, "insufficient Arc amount plus gas")

    completed = checkpoint["completed"]
    z3.same(len(completed), index, "complete ordered predecessor evidence")
    seen = set()
    for step, evidence in zip(steps[:index], completed):
        z3.same(evidence["step"], step.name, "completed step ordering")
        z3.same(evidence["command_sha256"], step.command_sha256, "completed command identity")
        z3.require(bool(evidence["artifacts"]), "partial artifact")
        for item in evidence["artifacts"]:
            raw = z3.artifact_bytes(Path(config["paths"]["packet_dir"]), item["path"])
            z3.same(hashlib.sha256(raw).hexdigest(), item["sha256"], "completed artifact hash")
        if step.kind in ("pftl", "arc"):
            receipt = evidence["receipt"]
            z3.same(receipt["accepted"], True, "previous rejected receipt")
            z3.same(receipt["finalized"], True, "previous missing finality")
            z3.require(receipt["tx_id"] not in seen, "duplicate/replay receipt")
            seen.add(receipt["tx_id"])
        elif step.kind == "replay_check":
            z3.same(evidence["rejected"], True, "release replay must reject")


def ensure_available(checkout: Path, step: Step, config: dict) -> None:
    z3.require((checkout / ARC_MARKER).is_file(),
               "Arc commands absent on this checkout; Arc steps require the operator-qualified lineage")
    source = subprocess.run(["git", "-C", str(checkout), "rev-parse", "HEAD"],
                            capture_output=True, text=True, check=True).stdout.strip()
    z3.same(source, config["manifest"]["source_commit"], "qualified checkout source commit")
    node = Path(config["paths"]["node_bin"])
    z3.require(node.is_file() and not z3.SECRET.search(str(node)), "qualified node binary absent")
    z3.same(hashlib.sha256(node.read_bytes()).hexdigest(),
            config["manifest"]["binary_sha256"], "qualified node binary hash")
    executable = step.argv[0]
    if "/" in executable:
        z3.require(Path(executable).is_file() and os.access(executable, os.X_OK),
                   "required command absent on current checkout")


def confirm_one(config: dict, checkpoint: dict, steps: list[Step], name: str,
                checkout: Path, runner: Callable = subprocess.run) -> dict:
    indexes = [i for i, step in enumerate(steps) if step.name == name]
    z3.require(len(indexes) == 1, "unknown confirmation step")
    index, = indexes
    step = steps[index]
    ensure_available(checkout, step, config)
    z3.require(not any(re.search(r"@[A-Z_]+@", arg) for arg in step.argv),
               "unresolved command input; retain readback and supply --values")
    check_checkpoint(config, checkpoint, steps, index)
    work = Path(config["paths"]["work_dir"])
    if name == "burn-bundle":
        redeem = z3.read_json(work / "redeem/redeem-manifest.json")
        amount = int(step.argv[step.argv.index("--amount-atoms") + 1])
        z3.same(amount, redeem["settlement_output_atoms"], "burn must use exact redeemed output")
    if name == "arc-release":
        for index, filename in ((4, "public-values.bin"), (5, "proof-calldata.bin")):
            raw = z3.artifact_bytes(work, "egress-proof/" + filename)
            z3.same(step.argv[index].lower(), "0x" + raw.hex(), "release proof bytes")
    if step.request:
        # This is a signer-local operation request containing paths, never a
        # signer file. Do not use the evidence loader, which rejects key_file.
        request_path = Path(step.request)
        z3.require(request_path.name.endswith(".ops.json"), "single operation request required")
        request = json.loads(request_path.read_text(encoding="utf-8"))
        z3.same(len(request["operations"]), 1, "one submission per confirmation")
        operation = request["operations"][0]["operation"]
        z3.same(operation["operation"], step.operation, "confirmed operation kind")
        ids = config["manifest"]["identities"]
        digest = hashlib.sha256(json.dumps(operation, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
        z3.same(digest, checkpoint["next_operation_sha256"], "reviewed next operation hash")
        if "asset_id" in operation:
            asset = ids["native_nav_asset_id"] if step.operation.startswith("nav_") else ids["settlement_asset_id"]
            z3.same(operation["asset_id"], asset, "operation asset")
        if "route_id" in operation:
            z3.same(operation["route_id"], ids["route_id"], "operation route")
        if "settlement_source_asset_id" in operation:
            z3.same(operation["settlement_source_asset_id"], ids["settlement_source_asset_id"], "operation source")
        if "settlement_asset_id" in operation:
            z3.same(operation["settlement_asset_id"], ids["settlement_asset_id"], "operation family")
    work = Path(config["paths"]["work_dir"])
    work.mkdir(parents=True, exist_ok=True, mode=0o700)
    attempts = work / "attempts"
    attempts.mkdir(exist_ok=True, mode=0o700)
    marker = attempts / (name + ".json")
    z3.write_new(marker, {"step": name, "command_sha256": step.command_sha256,
                          "inputs_sha256": metadata_hash(config), "state": "attempted"})
    identities_path = work / "reserve-identities.json"
    if not identities_path.exists():
        z3.write_new(identities_path, config["reserve_identities"])
    else:
        z3.same(z3.read_json(identities_path), config["reserve_identities"], "retained reserve identities")
    # Do not echo child output: an external signer may emit unsafe diagnostics.
    # Child stdout stays signer-local; publish only inspected public artifacts.
    with (work / (name + ".stdout.json")).open("x", encoding="utf-8") as output:
        result = runner(step.argv, cwd=checkout, stdout=output, stderr=subprocess.DEVNULL,
                        env={**os.environ, "PYTHONPATH": str(REPO / "python")},
                        check=False, timeout=config["parameters"]["latency_bound_seconds"])
    z3.require(result.returncode == 0 or step.kind == "replay_check",
               "step failed; attempt retained, campaign stopped")
    return {"verdict": "STOP", "executed_step": name, "returncode": result.returncode,
            "next": "retain and verify readbacks; a separate confirmation is required"}


def dry_run(config: dict, signers: dict, values: dict | None = None) -> dict:
    steps = compile_steps(config, signers, values)
    root = Path(config["paths"]["packet_dir"])
    root.mkdir(parents=True, exist_ok=True)
    layout = {"manifest": config["manifest"],
              **{section: {role: section + "/" + role + ".json" for role in roles}
                 for section, roles in z3.REQUIRED.items()}}
    z3.build_manifest(layout, root, root / "cycle.skeleton.json", skeleton=True)
    for step in steps:
        print(f"STOP [{step.name}] ({step.kind})")
        print(shlex.join(step.argv))
    return {"verdict": "DRY_RUN", "steps": len(steps),
            "skeleton": str(root / "cycle.skeleton.json"), "submissions": 0}


def write_request(output: Path, body: dict, source: str, signer: str) -> None:
    z3.require(z3.ACCOUNT.fullmatch(source) is not None, "full PFTL source required")
    z3.public(body)
    # A path string is the only signer input. Never stat/read/copy its target.
    output.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    with output.open("x", encoding="utf-8") as stream:
        json.dump({"schema": "postfiat-certified-asset-ops-request-v1",
                   "operations": [{"label": "z3-" + body["operation"], "source": source,
                                   "key_file": signer, "operation": body}]}, stream, indent=2)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    run = sub.add_parser("run")
    run.add_argument("--inputs", type=Path, required=True)
    run.add_argument("--checkout", type=Path, required=True)
    run.add_argument("--checkpoint", type=Path)
    run.add_argument("--values", type=Path)
    run.add_argument("--dry-run", action="store_true")
    run.add_argument("--confirm-step")
    for name in ("keystore", "password-file", "holder", "proposer", "finalizer", "issuer", "reserve", "settler"):
        run.add_argument("--" + name, required=True, help="signer-local path only; never read by this wrapper")
    check = sub.add_parser("check-preflight")
    check.add_argument("--inputs", type=Path, required=True)
    check.add_argument("--checkpoint", type=Path, required=True)
    wrap = sub.add_parser("wrap-operation")
    wrap.add_argument("--operation", type=Path, required=True)
    wrap.add_argument("--source", required=True)
    switch = sub.add_parser("route-switch")
    switch.add_argument("--route", required=True)
    switch.add_argument("--operator", required=True)
    switch.add_argument("--paused", choices=("true", "false"), required=True)
    for cmd in (wrap, switch):
        cmd.add_argument("--signer-path", required=True)
        cmd.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        if args.action in ("wrap-operation", "route-switch"):
            body = (z3.read_json(args.operation) if args.action == "wrap-operation" else
                    {"operation": "pftl_uniswap_route_pause", "operator": args.operator,
                     "route_id": args.route, "paused": args.paused == "true"})
            source = args.source if args.action == "wrap-operation" else args.operator
            write_request(args.output, body, source, args.signer_path)
            result = {"verdict": "PREPARED", "submissions": 0}
        else:
            config = z3.read_json(args.inputs)
            validate_inputs(config)
            if args.action == "check-preflight":
                checkpoint = z3.read_json(args.checkpoint)
                check_checkpoint(config, checkpoint, [Step("preflight", [])], 0)
                result = {"verdict": "PREFLIGHT_PASS", "scope": "frozen offline evidence"}
            else:
                signers = {name: getattr(args, name) for name in
                           ("keystore", "password_file", "holder", "proposer", "finalizer", "issuer", "reserve", "settler")}
                values = z3.read_json(args.values) if args.values else {}
                values.update(INPUTS=str(args.inputs), CHECKPOINT=str(args.checkpoint or "@CHECKPOINT@"))
                steps = compile_steps(config, signers, values)
                z3.require(not (args.dry_run and args.confirm_step), "dry-run cannot confirm a step")
                if args.dry_run:
                    result = dry_run(config, signers, values)
                elif args.confirm_step:
                    z3.require(args.checkpoint is not None, "checkpoint required before confirmation")
                    result = confirm_one(config, z3.read_json(args.checkpoint), steps,
                                         args.confirm_step, args.checkout)
                else:
                    checkpoint = z3.read_json(args.checkpoint) if args.checkpoint else None
                    index = len(checkpoint["completed"]) if checkpoint else 0
                    z3.require(index < len(steps), "all steps already evidenced")
                    step = steps[index]
                    print(shlex.join(step.argv))
                    ensure_available(args.checkout, step, config)
                    result = {"verdict": "STOP", "next_step": step.name,
                              "requires": "--confirm-step " + step.name}
        print(json.dumps(result, sort_keys=True))
        return 0
    except (z3.CycleError, KeyError, ValueError, TypeError, RuntimeError, OSError, subprocess.SubprocessError) as error:
        reason = str(error) if isinstance(error, z3.CycleError) else "missing or malformed input; stopped"
        print(json.dumps({"verdict": "BLOCKED", "error": reason}, sort_keys=True))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
