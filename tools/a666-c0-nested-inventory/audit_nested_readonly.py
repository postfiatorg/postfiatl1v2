#!/usr/bin/env python3
"""Read-only C3/C4 evidence for nested historical worktrees.

All source, diff, config, and process data is captured internally. Output is
limited to commit ids, code identifiers, sanitized locations, counts, hashes,
and finding classes. No matching source/config line or secret value is emitted.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import survey_nested as inventory  # noqa: E402


CURRENT_REPO = inventory.CURRENT_CANONICAL_REPO
CURRENT_COMMIT = inventory.CURRENT_CANONICAL
HOLDING = inventory.HOLDING
SAME_HISTORY_REF = inventory.SAME_HISTORY_REF
SCORE_CHILD = HOLDING / "postfiatl1v2-20260529-score-artifacts"

SYMBOL_RE = re.compile(
    rb"\b(?:fn|struct|enum|trait|type|const|static|mod|class|def)\s+"
    rb"([A-Za-z_][A-Za-z0-9_]*)"
)
SAFE_IDENTIFIER = re.compile(r"^[A-Za-z_][A-Za-z0-9_]{0,127}$")
SECURITY_IDENTIFIER_KEYWORDS = (
    "auth",
    "sign",
    "signature",
    "committee",
    "verify",
    "debug",
    "feature",
    "gate",
    "limit",
    "bound",
    "timeout",
    "semaphore",
    "integrity",
    "hmac",
    "checksum",
    "tamper",
    "circuit",
    "panic",
    "parse",
    "fuzz",
)
CONFIG_ROOTS: tuple[tuple[Path, str], ...] = (
    (Path("/home/postfiat/.config/systemd"), "user-systemd-config-reference"),
    (Path("/etc/systemd"), "system-systemd-config-reference"),
    (Path("/etc/caddy"), "caddy-config-reference"),
    (Path("/etc/cron.d"), "system-cron-config-reference"),
)


CANONICAL_BODY_TARGETS: tuple[tuple[str, str], ...] = (
    ("validate_rbc_signature_hex", "cobalt-signature-shape-validation"),
    ("validate_rbc_echo", "cobalt-rbc-echo-validation"),
    ("validate_rbc_propose", "cobalt-rbc-propose-validation"),
    ("rbc_validation_rejects_tampered_bindings", "cobalt-tamper-regression"),
    ("debug_proof_chain_id_allowed", "debug-proof-chain-allow-policy"),
    ("debug_proofs_enabled_for_chain", "debug-proof-chain-gate"),
    ("debug_shielded_pool_enabled_for_chain", "debug-pool-chain-gate"),
    (
        "debug_proof_gate_rejects_mainnet_chain_ids",
        "debug-mainnet-rejection-regression",
    ),
    (
        "debug_proof_gate_allows_explicit_debug_chain_ids",
        "debug-chain-allow-regression",
    ),
    ("validate_transport_envelope_auth", "transport-envelope-auth"),
    (
        "authenticated_health_exchange_binds_nonce_route_state_and_signers",
        "transport-auth-binding-regression",
    ),
    (
        "unsigned_owned_lane_mutations_are_never_remote_methods",
        "remote-unsigned-mutation-exclusion",
    ),
    ("rpc_serve_method_allowed", "remote-method-allow-policy"),
    ("try_committee", "panic-free-committee-parser"),
    ("committee", "committee-parser"),
    (
        "rpc_serve_rejects_oversized_request_lines_before_parse",
        "rpc-preparse-size-gate-regression",
    ),
    ("set_stream_timeout", "transport-stream-timeout"),
    ("snapshot_checksum", "storage-snapshot-checksum"),
    (
        "torn_final_record_is_ignored_but_checksum_corruption_is_fatal",
        "storage-corruption-regression",
    ),
    (
        "supported_asset_orchard_swap_circuit_id",
        "orchard-live-circuit-policy",
    ),
    (
        "validate_asset_orchard_swap_vk_policy",
        "orchard-live-vk-policy-validation",
    ),
    (
        "legacy_vk_ids_are_archive_only_at_the_verifier_policy_boundary",
        "orchard-legacy-vk-boundary-regression",
    ),
    ("fuzz_orchard_parser", "orchard-parser-fuzz-target"),
    (
        "asset_orchard_indexing_helpers_reject_count_mismatch_without_panic",
        "orchard-panic-free-index-regression",
    ),
)

MANAGER_SEMANTIC_RULINGS: tuple[dict[str, Any], ...] = (
    {
        "source": "pftl1v2-pr1-cobalt-sig-verify",
        "verdict": "keeper-port-required",
        "reason_class": "production-cobalt-signature-verification-absent",
        "evidence": (
            "Production RBC validation summaries call only schema/id/linked-message "
            "validators; cryptographic verifier references under the Cobalt crate "
            "occur only in examples/tests. Committee-binding equivalence is absent."
        ),
    },
    {
        "source": "pftl1v2-pr2-debug-pool-gate",
        "verdict": "semantic-close-archive-c4-candidate",
        "reason_class": "canonical-ci-and-exact-delegation-pass",
        "evidence": (
            "The exact-canonical proof-gate filter passes two tests; the privacy "
            "gate is an exact one-line delegation to that tested proof gate."
        ),
    },
    {
        "source": "pftl1v2-pr3-rpc-transport-auth",
        "verdict": "semantic-close-archive-c4-candidate",
        "reason_class": "canonical-transport-and-rpc-exclusion-ci-pass",
        "evidence": (
            "Exact-canonical transport binding and unsigned-remote-method exclusion "
            "filters each pass one test."
        ),
    },
    {
        "source": "pftl1v2-pr4-dos-hardening",
        "verdict": "keeper-review-or-port-required",
        "reason_class": "archived-concurrency-worker-and-parser-controls-unproved",
        "evidence": (
            "Canonical has pre-parse request limits and stream timeouts, but the "
            "archived global/per-peer dispatch bounds, bounded accept worker, and "
            "try_committee mechanism are absent by exact mechanism."
        ),
    },
    {
        "source": "pftl1v2-pr5-storage-integrity",
        "verdict": "keeper-port-required",
        "reason_class": "keyed-storage-integrity-absent",
        "evidence": (
            "Canonical snapshot/WAL integrity summaries use an unkeyed Sha3_384 "
            "checksum and corruption regression; archived IntegrityKey, HMAC, "
            "authenticated JSONL envelope, and keyed-open controls are absent."
        ),
    },
    {
        "source": "pftl1v2-pr6-orchard-vk-panics",
        "verdict": "semantic-close-archive-c4-candidate",
        "reason_class": "canonical-vk-and-panic-ci-pass",
        "evidence": (
            "Exact-canonical VK-boundary and panic-free indexing filters each pass "
            "one test; the Orchard parser fuzz target is structurally present."
        ),
    },
    {
        "source": "pftl1v2-pr9-ambient-backlog",
        "verdict": "canonical-supersedes-archive-c4-candidate",
        "reason_class": "complete-id-set-with-newer-status-classes",
        "evidence": (
            "All 198 finding ids persist and canonical intentionally reclassifies "
            "170 statuses; the newer canonical ledger is authoritative."
        ),
    },
    {
        "source": "pftl1v2-pr10a-dead-script-refs",
        "verdict": "patch-source-fresh-remediation-required",
        "reason_class": "current-dead-script-reference-backlog",
        "evidence": (
            "Canonical has 585 missing doc/script pairs across 102 active docs and "
            "361 unique absent script targets."
        ),
    },
    {
        "source": "pftl1v2-pr10b-docs-content-gaps",
        "verdict": "patch-source-fresh-rpc-coverage-required",
        "reason_class": "current-rpc-documentation-gap",
        "evidence": (
            "Canonical exposes 116 RPC dispatch wire methods; 62 are undocumented "
            "under docs/rpc."
        ),
    },
)

TARGETED_EXPECTATIONS: dict[str, tuple[tuple[str, str, str], ...]] = {
    "pftl1v2-pr1-cobalt-sig-verify": (
        ("identifier", "CobaltSignatureCommittee", "committee-binding-control"),
        ("identifier", "verify_cobalt_message_signature", "signature-verification-control"),
        ("identifier", "validate_rbc_echo_signed", "signed-rbc-validation"),
        (
            "identifier",
            "rbc_signed_validation_rejects_empty_forged_and_non_committee_signatures",
            "adversarial-signature-regression",
        ),
    ),
    "pftl1v2-pr2-debug-pool-gate": (
        ("literal", "debug-shielded-pool", "production-feature-gate"),
        ("literal", "debug-proofs", "production-feature-gate"),
        (
            "identifier",
            "debug_pool_gate_reflects_compile_time_feature",
            "compile-gate-regression",
        ),
        (
            "identifier",
            "debug_proof_gate_rejects_deployed_wan_devnet",
            "deployed-network-regression",
        ),
    ),
    "pftl1v2-pr3-rpc-transport-auth": (
        ("identifier", "RPC_AUTH_PROOF_SCHEMA", "rpc-auth-schema"),
        (
            "identifier",
            "validate_rpc_request_auth_proof",
            "rpc-auth-verification",
        ),
        (
            "identifier",
            "require_rpc_signing_request_authenticated",
            "signing-rpc-auth-gate",
        ),
        ("identifier", "TransportHelloAuth", "transport-auth-schema"),
        (
            "identifier",
            "validate_transport_hello_authenticated",
            "transport-auth-verification",
        ),
        (
            "identifier",
            "transport_hello_auth_rejects_replayed_stale_timestamp",
            "transport-replay-regression",
        ),
    ),
    "pftl1v2-pr4-dos-hardening": (
        (
            "identifier",
            "DEFAULT_RPC_CHILD_DISPATCH_CONCURRENT",
            "rpc-global-concurrency-bound",
        ),
        (
            "identifier",
            "try_acquire_rpc_serve_child_dispatch",
            "rpc-per-peer-concurrency-bound",
        ),
        (
            "identifier",
            "accept_transport_connections_bounded",
            "transport-worker-bound",
        ),
        ("identifier", "try_committee", "panic-free-bridge-parser"),
        (
            "identifier",
            "malformed_deposit_bytes_fixture_returns_error_instead_of_panicking",
            "malformed-input-regression",
        ),
    ),
    "pftl1v2-pr5-storage-integrity": (
        ("identifier", "IntegrityKey", "storage-integrity-key"),
        (
            "identifier",
            "open_with_integrity_key",
            "authenticated-store-open",
        ),
        ("identifier", "JSONL_ENVELOPE_KIND", "authenticated-jsonl-envelope"),
        (
            "identifier",
            "tampered_wal_mac_is_rejected_not_upgraded",
            "wal-tamper-regression",
        ),
        (
            "identifier",
            "tampered_state_file_is_rejected",
            "state-tamper-regression",
        ),
    ),
    "pftl1v2-pr6-orchard-vk-panics": (
        (
            "identifier",
            "assert_live_vk_is_current_release",
            "live-vk-policy-gate",
        ),
        (
            "identifier",
            "live_vk_selection_rejects_non_current_release_circuit",
            "live-vk-regression",
        ),
        (
            "identifier",
            "fuzz_asset_orchard_parser",
            "panic-free-parser-fuzz-target",
        ),
    ),
    "pftl1v2-pr9-ambient-backlog": (
        (
            "path",
            "docs/status/ambient-finding-disposition-ledger/p0-001-100.json",
            "ambient-disposition-ledger",
        ),
        (
            "path",
            "docs/status/ambient-finding-disposition-ledger/p0-101-198.json",
            "ambient-disposition-ledger",
        ),
    ),
    "pftl1v2-pr10a-dead-script-refs": (
        (
            "path",
            "docs/status/p0-and-docs-fix-checklist-20260725.md",
            "dead-reference-disposition-record",
        ),
        (
            "path",
            "docs/status/OPEN-SOURCE-PRODUCTIONIZATION-AUDIT-20260716.md",
            "productionization-audit",
        ),
    ),
    "pftl1v2-pr10b-docs-content-gaps": (
        (
            "path",
            "docs/rpc/method-coverage.md",
            "rpc-method-coverage",
        ),
        ("path", "mkdocs.yml", "documentation-navigation"),
    ),
}


def run_bytes(
    command: list[str],
    *,
    workdir: Path | None = None,
    allow_failure: bool = False,
) -> bytes | None:
    result = subprocess.run(
        command,
        cwd=workdir,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        if allow_failure:
            return None
        raise RuntimeError(f"captured command failed with exit class {result.returncode}")
    return result.stdout


def git_bytes(
    repo: Path, args: list[str], *, allow_failure: bool = False
) -> bytes | None:
    return run_bytes(
        ["git", "-C", str(repo), *args],
        allow_failure=allow_failure,
    )


def parse_name_status(repo: Path, base: str) -> list[dict[str, str]]:
    output = git_bytes(
        repo,
        ["diff", "--name-status", "-z", f"{base}..HEAD"],
    )
    assert output is not None
    parts = output.decode("utf-8", errors="surrogateescape").split("\0")
    records: list[dict[str, str]] = []
    index = 0
    while index < len(parts) and parts[index]:
        status = parts[index]
        index += 1
        if status.startswith(("R", "C")):
            old_path = parts[index]
            new_path = parts[index + 1]
            index += 2
            records.append(
                {
                    "status": status,
                    "old_location": inventory.safe_location(old_path),
                    "location": inventory.safe_location(new_path),
                    "_raw_location": new_path,
                }
            )
        else:
            path = parts[index]
            index += 1
            records.append(
                {
                    "status": status,
                    "location": inventory.safe_location(path),
                    "_raw_location": path,
                }
            )
    return records


def blob(repo: Path, ref: str, location: str) -> bytes | None:
    return git_bytes(
        repo,
        ["show", f"{ref}:{location}"],
        allow_failure=True,
    )


def normalized_added_lines(
    repo: Path, base: str, location: str
) -> tuple[set[bytes], set[str]]:
    output = git_bytes(
        repo,
        [
            "diff",
            "--no-ext-diff",
            "--no-color",
            "--unified=0",
            f"{base}..HEAD",
            "--",
            location,
        ],
    )
    assert output is not None
    lines: set[bytes] = set()
    symbols: set[str] = set()
    for raw in output.splitlines():
        if not raw.startswith(b"+") or raw.startswith(b"+++"):
            continue
        content = raw[1:].strip()
        for match in SYMBOL_RE.finditer(content):
            identifier = match.group(1).decode("ascii")
            if SAFE_IDENTIFIER.fullmatch(identifier):
                symbols.add(identifier)
        if (
            len(content) < 16
            or content.startswith((b"//", b"#", b"*", b"<!--"))
            or content in {b"{", b"}", b"(", b")", b"[", b"]"}
        ):
            continue
        normalized = b" ".join(content.split())
        lines.add(normalized)
    return lines, symbols


def canonical_symbol_locations(identifier: str) -> list[str]:
    output = git_bytes(
        CURRENT_REPO,
        [
            "grep",
            "-l",
            "-w",
            "-e",
            identifier,
            CURRENT_COMMIT,
            "--",
        ],
        allow_failure=True,
    )
    if output is None:
        return []
    prefix = f"{CURRENT_COMMIT}:"
    locations = []
    for line in output.decode("utf-8", errors="surrogateescape").splitlines():
        raw = line[len(prefix) :] if line.startswith(prefix) else line
        locations.append(inventory.safe_location(raw))
    return sorted(set(locations))




def targeted_carry_forward(worktree_name: str) -> dict[str, Any]:
    results = []
    for kind, expectation, control_class in TARGETED_EXPECTATIONS[worktree_name]:
        if kind == "path":
            present = blob(CURRENT_REPO, CURRENT_COMMIT, expectation) is not None
            locations = [inventory.safe_location(expectation)] if present else []
        else:
            output = git_bytes(
                CURRENT_REPO,
                [
                    "grep",
                    "-l",
                    "-F",
                    "-e",
                    expectation,
                    CURRENT_COMMIT,
                    "--",
                ],
                allow_failure=True,
            )
            locations = []
            if output is not None:
                prefix = f"{CURRENT_COMMIT}:"
                for line in output.decode(
                    "utf-8", errors="surrogateescape"
                ).splitlines():
                    raw = line[len(prefix) :] if line.startswith(prefix) else line
                    locations.append(inventory.safe_location(raw))
                locations = sorted(set(locations))
            present = bool(locations)
        results.append(
            {
                "kind": kind,
                "expectation": expectation,
                "control_class": control_class,
                "present_by_exact_name_or_path": present,
                "locations": locations,
            }
        )
    return {
        "methodology": (
            "A fixed allowlist of security-control identifiers, regression-test "
            "identifiers, feature names, and documentation paths is searched "
            "inside the current canonical commit. Output contains only the "
            "allowlisted identifier/path, sanitized result locations, booleans, "
            "and control classes; no matching line or value is emitted. Exact-name "
            "absence requires manual review and does not alone prove semantic loss."
        ),
        "expected": len(results),
        "present": sum(item["present_by_exact_name_or_path"] for item in results),
        "missing": sum(
            not item["present_by_exact_name_or_path"] for item in results
        ),
        "results": results,
    }


def semantic_audit(worktree: Path) -> dict[str, Any]:
    base = inventory.git(worktree, ["merge-base", SAME_HISTORY_REF, "HEAD"])
    assert base is not None
    changes = parse_name_status(worktree, base)
    exact = 0
    present = 0
    deleted_absent = 0
    total_added_lines = 0
    covered_added_lines = 0
    all_symbols: set[str] = set()
    canonical_by_location: dict[str, bytes] = {}
    file_results: list[dict[str, Any]] = []
    for change in changes:
        raw_location = change.pop("_raw_location")
        status = change["status"]
        branch_blob = None if status.startswith("D") else blob(worktree, "HEAD", raw_location)
        canonical_blob = blob(CURRENT_REPO, CURRENT_COMMIT, raw_location)
        if canonical_blob is not None:
            present += 1
            canonical_by_location[inventory.safe_location(raw_location)] = canonical_blob
        exact_blob = branch_blob is not None and branch_blob == canonical_blob
        if exact_blob:
            exact += 1
        if status.startswith("D") and canonical_blob is None:
            deleted_absent += 1
        added_lines, symbols = normalized_added_lines(worktree, base, raw_location)
        all_symbols.update(symbols)
        canonical_lines = (
            {b" ".join(line.strip().split()) for line in canonical_blob.splitlines()}
            if canonical_blob is not None
            else set()
        )
        covered = len(added_lines & canonical_lines)
        total_added_lines += len(added_lines)
        covered_added_lines += covered
        file_results.append(
            {
                **change,
                "canonical_path_present": canonical_blob is not None,
                "final_blob_exact": exact_blob,
                "distinct_added_lines": len(added_lines),
                "added_lines_present_same_path": covered,
                "introduced_identifiers": sorted(symbols),
            }
        )
    symbol_results = []
    for identifier in sorted(all_symbols):
        needle = re.compile(
            rb"\b" + re.escape(identifier.encode("ascii")) + rb"\b"
        )
        locations = sorted(
            location
            for location, data in canonical_by_location.items()
            if needle.search(data)
        )
        symbol_results.append(
            {
                "identifier": identifier,
                "present_in_corresponding_canonical_paths": bool(locations),
                "locations": locations,
            }
        )
    missing_symbols = [
        item["identifier"]
        for item in symbol_results
        if not item["present_in_corresponding_canonical_paths"]
    ]
    canonical_related: set[tuple[str, str]] = set()
    for location, data in canonical_by_location.items():
        for match in SYMBOL_RE.finditer(data):
            identifier = match.group(1).decode("ascii")
            lowered = identifier.lower()
            if any(keyword in lowered for keyword in SECURITY_IDENTIFIER_KEYWORDS):
                canonical_related.add((identifier, location))
    comparable_paths = sum(
        not item["status"].startswith("D") for item in file_results
    )
    line_ratio = (
        covered_added_lines / total_added_lines if total_added_lines else 1.0
    )
    if exact + deleted_absent == len(file_results):
        strength = "exact-tree-carry-forward"
    elif not missing_symbols and line_ratio >= 0.80:
        strength = "strong-structural-carry-forward"
    else:
        strength = "manual-c3-review-required"
    return {
        "worktree": str(worktree),
        "branch": inventory.git(worktree, ["branch", "--show-current"]),
        "head": inventory.git(worktree, ["rev-parse", "HEAD"]),
        "same_history_base": base,
        "current_canonical": {
            "repository": inventory.CURRENT_CANONICAL_LABEL,
            "commit": CURRENT_COMMIT,
            "history_relation": "no-common-ancestor",
        },
        "changed_paths": len(file_results),
        "comparable_non_deleted_paths": comparable_paths,
        "canonical_paths_present": present,
        "exact_final_blobs": exact,
        "deleted_paths_absent_from_canonical": deleted_absent,
        "distinct_added_lines": total_added_lines,
        "added_lines_present_same_path": covered_added_lines,
        "added_line_coverage_ratio": round(line_ratio, 6),
        "introduced_identifier_count": len(symbol_results),
        "missing_identifiers": missing_symbols,
        "canonical_related_identifiers": [
            {"identifier": identifier, "location": location}
            for identifier, location in sorted(canonical_related)
        ],
        "assessment": strength,
        "targeted_carry_forward": targeted_carry_forward(worktree.name),
        "assessment_limit": (
            "This read-only same-path structural audit is evidence for C3 "
            "routing, not a substitute for security review or passing canonical "
            "CI. Identifiers moved to a different path are conservatively marked "
            "missing and require manual C3 review."
        ),
        "files": file_results,
        "introduced_identifier_results": symbol_results,
    }


def canonical_body_summaries() -> list[dict[str, Any]]:
    summaries: list[dict[str, Any]] = []
    excluded_calls = {
        "if",
        "for",
        "while",
        "loop",
        "match",
        "return",
        "Some",
        "None",
        "Ok",
        "Err",
    }
    for identifier, control_class in CANONICAL_BODY_TARGETS:
        output = git_bytes(
            CURRENT_REPO,
            [
                "grep",
                "-l",
                "-F",
                "-e",
                f"fn {identifier}",
                CURRENT_COMMIT,
                "--",
                "*.rs",
            ],
            allow_failure=True,
        )
        locations: list[str] = []
        if output is not None:
            prefix = f"{CURRENT_COMMIT}:"
            for line in output.decode(
                "utf-8", errors="surrogateescape"
            ).splitlines():
                raw = line[len(prefix) :] if line.startswith(prefix) else line
                locations.append(raw)
        if not locations:
            summaries.append(
                {
                    "identifier": identifier,
                    "control_class": control_class,
                    "definition_found": False,
                    "locations": [],
                }
            )
            continue
        for raw_location in sorted(set(locations)):
            data = blob(CURRENT_REPO, CURRENT_COMMIT, raw_location)
            assert data is not None
            pattern = re.compile(
                rb"\bfn\s+" + re.escape(identifier.encode("ascii")) + rb"\s*\("
            )
            match = pattern.search(data)
            if match is None:
                continue
            brace = data.find(b"{", match.end())
            if brace < 0:
                continue
            depth = 0
            end = len(data)
            for index in range(brace, len(data)):
                byte = data[index]
                if byte == ord("{"):
                    depth += 1
                elif byte == ord("}"):
                    depth -= 1
                    if depth == 0:
                        end = index + 1
                        break
            signature = data[match.start() : brace]
            body = data[brace:end]
            calls = sorted(
                {
                    item.decode("ascii")
                    for item in re.findall(
                        rb"\b([A-Za-z_][A-Za-z0-9_]*)\s*!?\s*\(",
                        body,
                    )
                    if item.decode("ascii") not in excluded_calls
                }
            )
            constants = sorted(
                {
                    item.decode("ascii")
                    for item in re.findall(rb"\b[A-Z][A-Z0-9_]{2,}\b", body)
                }
            )
            types = sorted(
                {
                    item.decode("ascii")
                    for item in re.findall(
                        rb"\b[A-Z][A-Za-z0-9_]{2,}\b", body
                    )
                    if not re.fullmatch(rb"[A-Z][A-Z0-9_]{2,}", item)
                }
            )
            parameters = sorted(
                {
                    item.decode("ascii")
                    for item in re.findall(
                        rb"\b([a-z_][A-Za-z0-9_]*)\s*:", signature
                    )
                }
            )
            lowered = body.lower()
            risk_counts = {
                keyword: lowered.count(keyword.encode())
                for keyword in (
                    "auth",
                    "verify",
                    "signature",
                    "committee",
                    "chain_id",
                    "mainnet",
                    "limit",
                    "timeout",
                    "checksum",
                    "panic",
                    "unwrap",
                    "expect",
                    "unsafe",
                )
            }
            summaries.append(
                {
                    "identifier": identifier,
                    "control_class": control_class,
                    "definition_found": True,
                    "locations": [inventory.safe_location(raw_location)],
                    "parameter_identifiers": parameters,
                    "called_identifiers": calls,
                    "referenced_constants": constants,
                    "referenced_type_identifiers": types,
                    "risk_keyword_counts": risk_counts,
                    "body_bytes": len(body),
                    "body_sha256": hashlib.sha256(body).hexdigest(),
                }
            )
    return summaries


def config_references(targets: list[Path]) -> list[dict[str, str]]:
    findings: set[tuple[str, str, str]] = set()
    encoded_targets = [(str(target).encode(), target.name) for target in targets]
    for root, finding_class in CONFIG_ROOTS:
        if not root.exists():
            continue
        for path in root.rglob("*"):
            if not path.is_file() or path.is_symlink():
                continue
            try:
                if path.stat().st_size > inventory.MAX_CONTENT_SCAN_BYTES:
                    continue
                data = path.read_bytes()
            except OSError:
                continue
            for encoded, name in encoded_targets:
                if encoded in data:
                    findings.add(
                        (
                            inventory.safe_location(str(path)),
                            name,
                            finding_class,
                        )
                    )
    return [
        {"location": location, "target": target, "class": finding_class}
        for location, target, finding_class in sorted(findings)
    ]


def process_references(targets: list[Path]) -> list[dict[str, Any]]:
    target_strings = [(str(target), target.name) for target in targets]
    findings: set[tuple[int, str, str]] = set()
    proc = Path("/proc")
    for pid_path in proc.iterdir():
        if not pid_path.name.isdigit():
            continue
        pid = int(pid_path.name)
        probes: list[tuple[Path, str]] = [
            (pid_path / "cwd", "process-cwd-reference"),
            (pid_path / "exe", "process-executable-reference"),
        ]
        fd_root = pid_path / "fd"
        try:
            probes.extend(
                (fd_path, "process-open-fd-reference")
                for fd_path in fd_root.iterdir()
            )
        except OSError:
            pass
        for probe, finding_class in probes:
            try:
                resolved = os.readlink(probe)
            except OSError:
                continue
            for target, name in target_strings:
                if resolved == target or resolved.startswith(target + "/"):
                    findings.add((pid, name, finding_class))
    return [
        {"pid": pid, "target": target, "class": finding_class}
        for pid, target, finding_class in sorted(findings)
    ]


def hash_score_artifacts() -> dict[str, Any]:
    files = inventory.candidate_plain_files(SCORE_CHILD)
    entries = []
    total = 0
    for relative in files:
        path = SCORE_CHILD / relative
        data = path.read_bytes()
        size = len(data)
        total += size
        entries.append(
            {
                "location": inventory.safe_location(relative),
                "bytes": size,
                "sha256": hashlib.sha256(data).hexdigest(),
            }
        )
    return {
        "schema": "postfiat.track-c.content-hash-manifest.v1",
        "root": str(SCORE_CHILD),
        "methodology": (
            "Only sanitized locations, byte counts, and SHA-256 content hashes "
            "are emitted; file contents and matched secret-adjacent values are "
            "never emitted."
        ),
        "file_count": len(entries),
        "content_bytes": total,
        "files": entries,
    }


def atomic_write(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    os.replace(temporary, path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--semantic-output", type=Path, required=True)
    parser.add_argument("--hash-output", type=Path, required=True)
    args = parser.parse_args()
    git_children = sorted(
        HOLDING / name for name in inventory.PR_METADATA
    )
    audits = [semantic_audit(path) for path in git_children]
    all_targets = git_children + [SCORE_CHILD]
    runtime = {
        "methodology": (
            "Config and proc data are read internally. Output contains only "
            "sanitized config locations, target directory names, PIDs, and "
            "reference classes; no matching config line, environment, command "
            "line, file content, or secret value is emitted."
        ),
        "config_references": config_references(all_targets),
        "process_references": process_references(all_targets),
    }
    semantic = {
        "schema": "postfiat.track-c.nested-semantic-audit.v1",
        "current_canonical": {
            "repository": inventory.CURRENT_CANONICAL_LABEL,
            "commit": CURRENT_COMMIT,
        },
        "methodology": (
            "Old-branch diffs and current-canonical blobs are captured "
            "internally. Output is limited to commit ids, code identifiers, "
            "sanitized locations, counts, ratios, and assessment classes; no "
            "source line, diff hunk, config line, or secret value is emitted."
        ),
        "audits": audits,
        "canonical_control_body_summaries": canonical_body_summaries(),
        "manager_semantic_rulings": list(MANAGER_SEMANTIC_RULINGS),
        "retirement_reference_audit": runtime,
    }
    hashes = hash_score_artifacts()
    atomic_write(args.semantic_output, semantic)
    atomic_write(args.hash_output, hashes)
    summary = {
        "audits": len(audits),
        "assessment_counts": {
            assessment: sum(
                item["assessment"] == assessment for item in audits
            )
            for assessment in sorted({item["assessment"] for item in audits})
        },
        "config_references": len(runtime["config_references"]),
        "process_references": len(runtime["process_references"]),
        "score_hash_entries": hashes["file_count"],
        "output_policy": "locations-identifiers-counts-classes-hashes-only",
    }
    print(json.dumps(summary, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
