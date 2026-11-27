#!/usr/bin/env python3
"""strfry writePolicy plugin for relay.postfiat.org (Pillar 6 / Contract C3).

Reads newline-delimited JSON event admission requests from stdin and writes
strfry policy responses to stdout:
  {"id": "<event_id>", "action": "accept" | "reject" | "shadowReject", "msg": "..."}

Enforces:
  1. Registered-member write admission & NIP-42 authenticated pubkey matching.
  2. Contract C3 Nostr event kind allowlist ([0, 1, 13, 1059, 10050]; kind 22242
     is NIP-42 auth handshake only and never persisted).
  3. Non-production kind:0 publish block (when NOSTR_RELAY_ENV != "production").
  4. Event shape & timestamp bounds (content <= 32768 chars, tags <= 64,
     created_at within now +/- 600s except NIP-17 kind:1059 gift wraps).
  5. Inbound mirror filtering (sourceType in {"Import", "Stream", "Sync"}):
     unregistered authors are shadow-rejected so public-relay spam floods never
     fill relay.postfiat.org.
  6. Per-pubkey sliding-window rate limits (distinct budgets for direct client
     writes vs inbound mirror sync).
"""

from __future__ import annotations

import collections
import json
import os
import re
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any


HEX64_RE = re.compile(r"^[0-9a-f]{64}$")

# Contract C3 allowlisted persistent Nostr event kinds
DEFAULT_ALLOWED_KINDS = frozenset({0, 1, 13, 1059, 10050})
NIP42_AUTH_KIND = 22242
NIP17_GIFT_WRAP_KIND = 1059

MAX_CONTENT_CHARS = 32_768
MAX_TAGS = 64
MAX_TAG_ELEMENT_CHARS = 1_024
MAX_CLOCK_SKEW_SECONDS = 600

MIRROR_SOURCE_TYPES = frozenset({"Import", "Stream", "Sync"})


@dataclass
class RelayPolicyConfig:
    env: str = "production"
    allowlist_path: Path | None = None
    allowed_pubkeys: frozenset[str] = frozenset()
    allowed_kinds: frozenset[int] = DEFAULT_ALLOWED_KINDS
    require_nip42_auth: bool = True
    rate_limit_max_events: int = 20
    rate_limit_window_seconds: int = 60
    mirror_rate_limit_max_events: int = 40
    max_content_chars: int = MAX_CONTENT_CHARS
    max_tags: int = MAX_TAGS
    max_clock_skew_seconds: int = MAX_CLOCK_SKEW_SECONDS


class AllowlistLoader:
    """Reloads registered member pubkeys from JSON file whenever mtime changes."""

    def __init__(self, path: Path | None, static_pubkeys: frozenset[str] = frozenset()) -> None:
        self._path = path
        self._static = {pk.lower() for pk in static_pubkeys if HEX64_RE.match(pk.lower())}
        self._mtime_ns: int | None = None
        self._loaded: set[str] = set()

    def current(self) -> set[str]:
        if self._path is None:
            return set(self._static)
        try:
            stat = self._path.stat()
            if stat.st_mtime_ns != self._mtime_ns:
                raw = json.loads(self._path.read_text(encoding="utf-8"))
                keys = raw.get("registered_pubkeys", []) if isinstance(raw, dict) else []
                self._loaded = {
                    str(k).strip().lower()
                    for k in keys
                    if isinstance(k, str) and HEX64_RE.match(k.strip().lower())
                }
                self._mtime_ns = stat.st_mtime_ns
        except (OSError, ValueError):
            # Keep last known good allowlist on transient read/parse failure
            pass
        return self._static | self._loaded


class WritePolicyEngine:
    def __init__(self, config: RelayPolicyConfig, *, now_fn=time.time) -> None:
        self.config = config
        self._now = now_fn
        self._allowlist = AllowlistLoader(config.allowlist_path, config.allowed_pubkeys)
        self._client_windows: dict[str, collections.deque[float]] = {}
        self._mirror_windows: dict[str, collections.deque[float]] = {}

    def _admit_rate(self, buckets: dict[str, collections.deque[float]], key: str, limit: int, now_s: float) -> bool:
        window = float(self.config.rate_limit_window_seconds)
        dq = buckets.setdefault(key, collections.deque())
        cutoff = now_s - window
        while dq and dq[0] <= cutoff:
            dq.popleft()
        if len(dq) >= limit:
            return False
        dq.append(now_s)
        return True

    def evaluate(self, request: dict[str, Any]) -> dict[str, str]:
        event = request.get("event") if isinstance(request, dict) else None
        if not isinstance(event, dict):
            return {"id": "", "action": "reject", "msg": "invalid: missing event object"}

        event_id = str(event.get("id", ""))
        pubkey = str(event.get("pubkey", "")).lower()
        kind = event.get("kind")
        content = event.get("content")
        tags = event.get("tags")
        created_at = event.get("created_at")
        source_type = str(request.get("sourceType", "IP4"))
        is_mirror = source_type in MIRROR_SOURCE_TYPES

        def reject(msg: str, *, shadow_if_mirror: bool = False) -> dict[str, str]:
            action = "shadowReject" if (shadow_if_mirror and is_mirror) else "reject"
            return {"id": event_id, "action": action, "msg": msg}

        if not HEX64_RE.match(event_id) or not HEX64_RE.match(pubkey):
            return reject("invalid: malformed event id or pubkey", shadow_if_mirror=True)

        if not isinstance(kind, int) or isinstance(kind, bool):
            return reject("invalid: kind must be integer", shadow_if_mirror=True)

        if kind == NIP42_AUTH_KIND:
            return reject("restricted: NIP-42 auth events are ephemeral and not stored", shadow_if_mirror=True)

        if kind not in self.config.allowed_kinds:
            return reject(
                f"restricted: event kind {kind} is not on relay.postfiat.org allowlist",
                shadow_if_mirror=True,
            )

        if kind == 0 and self.config.env.lower() != "production":
            return reject(
                "restricted: kind:0 profile publishing blocked in non-production",
                shadow_if_mirror=True,
            )

        if not isinstance(content, str) or len(content) > self.config.max_content_chars:
            return reject("invalid: content missing or exceeds size limit", shadow_if_mirror=True)

        if not isinstance(tags, list) or len(tags) > self.config.max_tags:
            return reject("invalid: tags missing or exceed tag count limit", shadow_if_mirror=True)
        for tag in tags:
            if not isinstance(tag, list) or any(
                not isinstance(el, str) or len(el) > MAX_TAG_ELEMENT_CHARS for el in tag
            ):
                return reject("invalid: malformed tag element", shadow_if_mirror=True)

        now_s = float(request.get("receivedAt") or self._now())
        if not isinstance(created_at, int) or isinstance(created_at, bool):
            return reject("invalid: created_at must be integer", shadow_if_mirror=True)
        if kind != NIP17_GIFT_WRAP_KIND:
            if abs(now_s - created_at) > self.config.max_clock_skew_seconds:
                return reject("invalid: created_at outside allowed clock skew window", shadow_if_mirror=True)

        registered = self._allowlist.current()

        if is_mirror:
            # Inbound mirror filter: only mirror registered Post Fiat members
            # (or NIP-17 gift wraps explicitly addressed via 'p' tag to a registered member).
            addressed_to_member = False
            if kind == NIP17_GIFT_WRAP_KIND:
                for tag in tags:
                    if len(tag) >= 2 and tag[0] == "p" and tag[1].lower() in registered:
                        addressed_to_member = True
                        break
            if pubkey not in registered and not addressed_to_member:
                return reject("blocked: inbound mirror event not associated with registered member", shadow_if_mirror=True)
            if not self._admit_rate(self._mirror_windows, pubkey, self.config.mirror_rate_limit_max_events, now_s):
                return reject("rate-limited: inbound mirror per-pubkey rate exceeded", shadow_if_mirror=True)
            return {"id": event_id, "action": "accept", "msg": ""}

        # Direct client write: enforce NIP-42 authenticated pubkey & registered-member allowlist
        if self.config.require_nip42_auth:
            auth_pubkey = str(request.get("authedPubkey") or request.get("authPubkey") or "").lower()
            if not HEX64_RE.match(auth_pubkey):
                return reject("auth-required: NIP-42 authentication required to publish")
            if auth_pubkey not in registered:
                return reject("restricted: authenticated pubkey is not a registered Post Fiat member")
            # For non-gift-wrap events, authenticated pubkey must match event author
            if kind != NIP17_GIFT_WRAP_KIND and auth_pubkey != pubkey:
                return reject("restricted: NIP-42 authenticated pubkey must match event pubkey")
        elif pubkey not in registered:
            return reject("restricted: pubkey is not a registered Post Fiat member")

        if not self._admit_rate(self._client_windows, pubkey, self.config.rate_limit_max_events, now_s):
            return reject("rate-limited: exceeded per-pubkey event rate limit")

        return {"id": event_id, "action": "accept", "msg": ""}


def load_config_from_env() -> RelayPolicyConfig:
    allowlist_raw = os.environ.get("NOSTR_RELAY_ALLOWLIST_PATH", "").strip()
    env = os.environ.get("NOSTR_RELAY_ENV", "production").strip() or "production"
    require_auth = os.environ.get("NOSTR_RELAY_REQUIRE_NIP42", "true").strip().lower() != "false"
    max_events = int(os.environ.get("NOSTR_RELAY_RATE_LIMIT_MAX_EVENTS", "20"))
    window_s = int(os.environ.get("NOSTR_RELAY_RATE_LIMIT_WINDOW_SECONDS", "60"))
    mirror_max = int(os.environ.get("NOSTR_RELAY_MIRROR_RATE_LIMIT_MAX_EVENTS", "40"))
    return RelayPolicyConfig(
        env=env,
        allowlist_path=Path(allowlist_raw) if allowlist_raw else None,
        require_nip42_auth=require_auth,
        rate_limit_max_events=max_events,
        rate_limit_window_seconds=window_s,
        mirror_rate_limit_max_events=mirror_max,
    )


def main() -> int:
    engine = WritePolicyEngine(load_config_from_env())
    for line in sys.stdin:
        text = line.strip()
        if not text:
            continue
        try:
            req = json.loads(text)
        except json.JSONDecodeError:
            resp = {"id": "", "action": "reject", "msg": "invalid: malformed JSON input"}
        else:
            if req.get("type") != "new":
                continue
            resp = engine.evaluate(req)
        sys.stdout.write(json.dumps(resp, separators=(",", ":")) + "\n")
        sys.stdout.flush()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
