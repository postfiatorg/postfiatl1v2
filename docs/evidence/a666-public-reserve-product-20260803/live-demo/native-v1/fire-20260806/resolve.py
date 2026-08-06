#!/usr/bin/env python3
"""Deterministically resolve staged fire packets without touching HELD sources."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import re
import shutil
from pathlib import Path
from typing import Any, Mapping

ROOT = Path(__file__).resolve().parent
BINDING_NAME = "authorization-binding-native-v1.json"
STAGE_RE = re.compile(r"[A-Za-z0-9_-]+")


class ResolutionError(RuntimeError):
    """Raised for invalid staged-resolution inputs."""


def _canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")


def _sha256_path(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _load_object(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise ResolutionError(f"invalid JSON: {path}") from exc
    if not isinstance(value, dict):
        raise ResolutionError(f"JSON root must be an object: {path}")
    return value


def _write_canonical(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(_canonical_bytes(value))


def _pointer_parts(pointer: str) -> list[str]:
    if not isinstance(pointer, str) or not pointer.startswith("/") or pointer == "/":
        raise ResolutionError(f"resolution pointer must be a non-root JSON pointer: {pointer!r}")
    return [part.replace("~1", "/").replace("~0", "~") for part in pointer[1:].split("/")]


def _get_pointer(root: Any, pointer: str) -> Any:
    current = root
    for part in _pointer_parts(pointer):
        if isinstance(current, dict):
            if part not in current:
                raise ResolutionError(f"resolution pointer does not exist: {pointer}")
            current = current[part]
        elif isinstance(current, list):
            try:
                index = int(part)
            except ValueError as exc:
                raise ResolutionError(f"list index is invalid in pointer: {pointer}") from exc
            if index < 0 or index >= len(current):
                raise ResolutionError(f"list index is out of range in pointer: {pointer}")
            current = current[index]
        else:
            raise ResolutionError(f"resolution pointer crosses scalar value: {pointer}")
    return current


def _set_pointer(root: Any, pointer: str, value: Any) -> None:
    parts = _pointer_parts(pointer)
    current = root
    for part in parts[:-1]:
        if isinstance(current, dict):
            if part not in current:
                raise ResolutionError(f"resolution pointer does not exist: {pointer}")
            current = current[part]
        elif isinstance(current, list):
            try:
                index = int(part)
            except ValueError as exc:
                raise ResolutionError(f"list index is invalid in pointer: {pointer}") from exc
            if index < 0 or index >= len(current):
                raise ResolutionError(f"list index is out of range in pointer: {pointer}")
            current = current[index]
        else:
            raise ResolutionError(f"resolution pointer crosses scalar value: {pointer}")
    final = parts[-1]
    if isinstance(current, dict):
        if final not in current:
            raise ResolutionError(f"resolution pointer does not exist: {pointer}")
        current[final] = value
    elif isinstance(current, list):
        try:
            index = int(final)
        except ValueError as exc:
            raise ResolutionError(f"list index is invalid in pointer: {pointer}") from exc
        if index < 0 or index >= len(current):
            raise ResolutionError(f"list index is out of range in pointer: {pointer}")
        current[index] = value
    else:
        raise ResolutionError(f"resolution pointer crosses scalar value: {pointer}")


def _repo_root(path: Path) -> Path:
    for candidate in (path, *path.parents):
        if (candidate / ".git").exists():
            return candidate
    raise ResolutionError(f"unable to find repository root from {path}")


def _relative_packet_path(path: Path) -> str:
    return path.relative_to(_repo_root(ROOT)).as_posix()


def _stage_name(value: Any) -> str:
    if not isinstance(value, str) or not STAGE_RE.fullmatch(value):
        raise ResolutionError("stage must match [A-Za-z0-9_-]+")
    return value


def _stage_packets(source_dir: Path, values: Mapping[str, Any]) -> list[str]:
    active = values.get("active_packets")
    if not isinstance(active, list) or not active:
        raise ResolutionError("active_packets must be a non-empty array")
    names: list[str] = []
    for name in active:
        if not isinstance(name, str) or Path(name).name != name or not name.startswith("native-leg") or not name.endswith(".json"):
            raise ResolutionError(f"invalid packet name: {name!r}")
        if not (source_dir / name).is_file():
            raise ResolutionError(f"HELD packet does not exist: {name}")
        names.append(name)
    if len(names) != len(set(names)):
        raise ResolutionError("active_packets contains duplicates")
    return sorted(names)


def _resolved_values(values: Mapping[str, Any]) -> Mapping[str, Any]:
    resolved = values.get("resolved_values", {})
    if not isinstance(resolved, dict):
        raise ResolutionError("resolved_values must be an object")
    for filename, fields in resolved.items():
        if not isinstance(filename, str) or not isinstance(fields, dict):
            raise ResolutionError("resolved_values must map packet filenames to pointer/value objects")
    return resolved


def _binding_context(values: Mapping[str, Any]) -> Mapping[str, Any]:
    context = values.get("binding_context", {})
    if not isinstance(context, dict):
        raise ResolutionError("binding_context must be an object")
    return context


def render_stage(source_dir: Path, values: Mapping[str, Any], output_root: Path) -> tuple[Path, Path]:
    stage = _stage_name(values.get("stage"))
    source_dir = source_dir.resolve()
    output_root = output_root.resolve()
    root_resolved = ROOT.resolve()
    if output_root != root_resolved and root_resolved not in output_root.parents:
        raise ResolutionError("output_root must be fire-20260806 or one of its descendants")
    source_binding_path = source_dir / BINDING_NAME
    source_binding = _load_object(source_binding_path)
    packets = _stage_packets(source_dir, values)
    replacements = _resolved_values(values)
    unknown = set(replacements).difference(packets)
    if unknown:
        raise ResolutionError(f"resolved_values names are not active packets: {sorted(unknown)}")

    packet_dir = output_root / f"packets-{stage}"
    binding_entries: list[dict[str, str]] = []
    for name in packets:
        source_path = source_dir / name
        source = _load_object(source_path)
        resolved = copy.deepcopy(source)
        diffs: list[dict[str, Any]] = []
        for pointer in sorted(replacements.get(name, {})):
            new_value = copy.deepcopy(replacements[name][pointer])
            old_value = copy.deepcopy(_get_pointer(resolved, pointer))
            _set_pointer(resolved, pointer, new_value)
            diffs.append({"json_pointer": pointer, "source_value": old_value, "resolved_value": new_value})
        resolved["source_packet_sha256"] = _sha256_path(source_path)
        resolved["resolution_stage"] = stage
        resolved["resolved_fields"] = diffs
        output_packet = packet_dir / name
        _write_canonical(output_packet, resolved)
        binding_entries.append({"path": _relative_packet_path(output_packet), "sha256": _sha256_path(output_packet)})

    binding = copy.deepcopy(source_binding)
    binding["binding_status"] = f"STAGED-{stage}-PENDING-FIRE-TIME-AUTHORIZATION"
    binding["source_binding_sha256"] = _sha256_path(source_binding_path)
    binding["stage"] = stage
    binding["stage_packet_count"] = len(binding_entries)
    binding["stage_resolution_context"] = copy.deepcopy(_binding_context(values))
    binding["packets"] = binding_entries
    binding_path = output_root / f"binding-{stage}.json"
    _write_canonical(binding_path, binding)
    return packet_dir, binding_path


def _tree_hashes(directory: Path) -> dict[str, str]:
    return {
        path.relative_to(directory).as_posix(): _sha256_path(path)
        for path in sorted(directory.rglob("*"))
        if path.is_file()
    }


def self_check() -> None:
    source_dir = ROOT.parent
    work = ROOT / ".self-check"
    if work.exists():
        shutil.rmtree(work)
    values: dict[str, Any] = {
        "stage": "SELF-CHECK",
        "active_packets": ["native-leg2a-order-reserve.json"],
        "resolved_values": {
            "native-leg2a-order-reserve.json": {
                "/ops_file_template/operations/0/operation/expires_at_height": 1776
            }
        },
    }
    try:
        render_stage(source_dir, values, work)
        first = _tree_hashes(work)
        render_stage(source_dir, values, work)
        second = _tree_hashes(work)
        if first != second:
            raise ResolutionError("determinism self-check failed: output SHA maps differ")
        print(f"SELF_CHECK PASS files={len(first)} deterministic_sha_maps_match=true")
    finally:
        if work.exists():
            shutil.rmtree(work)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--held-dir", type=Path, default=ROOT.parent, help="native-v1 HELD packet directory")
    parser.add_argument("--values", type=Path, help="JSON stage values file")
    parser.add_argument("--output-root", type=Path, default=ROOT, help="fire-20260806 output root or descendant")
    parser.add_argument("--self-check", action="store_true", help="render identical synthetic inputs twice and compare SHA maps")
    args = parser.parse_args()
    if args.self_check:
        if args.values is not None:
            parser.error("--self-check does not accept --values")
        self_check()
        return
    if args.values is None:
        parser.error("--values is required unless --self-check is used")
    values = _load_object(args.values)
    packet_dir, binding_path = render_stage(args.held_dir, values, args.output_root)
    print(json.dumps({"binding": str(binding_path), "packets_dir": str(packet_dir), "binding_sha256": _sha256_path(binding_path)}, sort_keys=True))


if __name__ == "__main__":
    main()
