"""London-style fixing window scheduler, cutoff enforcement, and duplicate-window guard."""

from __future__ import annotations

import fcntl
import json
import os
import tempfile
from dataclasses import dataclass
from datetime import date, datetime, time, timezone
from pathlib import Path
from typing import Any
from zoneinfo import ZoneInfo

LONDON_TZ_NAME = "Europe/London"
STATE_SCHEMA_V1 = "postfiat.fx_fix.window_state.v1"


@dataclass(frozen=True)
class WindowScheduleConfig:
    """Schedule parameters for London-style deterministic FX fixing windows."""

    timezone_name: str = LONDON_TZ_NAME
    fix_hour: int = 16
    fix_minute: int = 0
    cutoff_seconds_before_fix: int = 60
    max_lateness_seconds: int = 120
    weekdays_only: bool = True
    holiday_dates_iso: tuple[str, ...] = ()

    def validate(self) -> None:
        ZoneInfo(self.timezone_name)
        if not (0 <= self.fix_hour <= 23 and 0 <= self.fix_minute <= 59):
            raise ValueError("fix_hour must be 0..=23 and fix_minute must be 0..=59")
        if self.cutoff_seconds_before_fix < 0:
            raise ValueError("cutoff_seconds_before_fix must be >= 0")
        if self.max_lateness_seconds < 0:
            raise ValueError("max_lateness_seconds must be >= 0")
        for iso_day in self.holiday_dates_iso:
            date.fromisoformat(iso_day)


@dataclass(frozen=True)
class FixWindow:
    """Resolved fixing window with exact UTC and London timestamps."""

    window_id: str
    timezone_name: str
    local_date: str
    fix_time_local_iso: str
    fix_time_utc_iso: str
    fix_timestamp_unix: int
    cutoff_timestamp_unix: int
    deadline_timestamp_unix: int


@dataclass(frozen=True)
class CutoffStatus:
    """Result of evaluating execution time against a London fixing window."""

    allowed: bool
    reason: str
    seconds_from_fix: int
    window: FixWindow


def resolve_london_window(
    now_utc: datetime,
    schedule: WindowScheduleConfig,
    *,
    target_local_date: str | None = None,
) -> FixWindow:
    """Resolve the exact DST-aware fixing window in ``schedule.timezone_name``."""
    schedule.validate()
    if now_utc.tzinfo is None:
        raise ValueError("now_utc must be timezone-aware")
    tz = ZoneInfo(schedule.timezone_name)
    local_now = now_utc.astimezone(tz)
    local_day = date.fromisoformat(target_local_date) if target_local_date else local_now.date()

    fix_local = datetime.combine(
        local_day,
        time(schedule.fix_hour, schedule.fix_minute, 0),
        tzinfo=tz,
    )
    fix_utc = fix_local.astimezone(timezone.utc)
    fix_ts = int(fix_utc.timestamp())
    cutoff_ts = fix_ts - schedule.cutoff_seconds_before_fix
    deadline_ts = fix_ts + schedule.max_lateness_seconds
    window_id = f"{local_day.isoformat()}T{schedule.fix_hour:02d}{schedule.fix_minute:02d}-{schedule.timezone_name}"

    return FixWindow(
        window_id=window_id,
        timezone_name=schedule.timezone_name,
        local_date=local_day.isoformat(),
        fix_time_local_iso=fix_local.isoformat(),
        fix_time_utc_iso=fix_utc.isoformat(),
        fix_timestamp_unix=fix_ts,
        cutoff_timestamp_unix=cutoff_ts,
        deadline_timestamp_unix=deadline_ts,
    )


def evaluate_cutoff(
    now_utc: datetime,
    schedule: WindowScheduleConfig,
    *,
    target_local_date: str | None = None,
    allow_before_fix: bool = False,
) -> CutoffStatus:
    """Enforce London fixing window calendar, cutoff, and lateness boundaries.

    - Rejects weekends (if ``weekdays_only``) and configured market holidays.
    - Order intake closes at ``cutoff_timestamp_unix = fix_ts - cutoff_seconds_before_fix``.
    - Execution must run at or after ``fix_timestamp_unix`` (unless ``allow_before_fix``
      is explicitly set for pre-fix dry-run preview after cutoff) and no later than
      ``deadline_timestamp_unix = fix_ts + max_lateness_seconds``.
    """
    window = resolve_london_window(now_utc, schedule, target_local_date=target_local_date)
    local_day = date.fromisoformat(window.local_date)
    now_ts = int(now_utc.astimezone(timezone.utc).timestamp())
    seconds_from_fix = now_ts - window.fix_timestamp_unix

    if schedule.weekdays_only and local_day.weekday() >= 5:
        return CutoffStatus(
            allowed=False,
            reason=f"weekend_closed:{window.local_date}",
            seconds_from_fix=seconds_from_fix,
            window=window,
        )
    if window.local_date in schedule.holiday_dates_iso:
        return CutoffStatus(
            allowed=False,
            reason=f"holiday_closed:{window.local_date}",
            seconds_from_fix=seconds_from_fix,
            window=window,
        )
    if now_ts < window.cutoff_timestamp_unix:
        return CutoffStatus(
            allowed=False,
            reason=f"before_order_cutoff:remaining={window.cutoff_timestamp_unix - now_ts}s",
            seconds_from_fix=seconds_from_fix,
            window=window,
        )
    if not allow_before_fix and now_ts < window.fix_timestamp_unix:
        return CutoffStatus(
            allowed=False,
            reason=f"before_fix_time:remaining={window.fix_timestamp_unix - now_ts}s",
            seconds_from_fix=seconds_from_fix,
            window=window,
        )
    if now_ts > window.deadline_timestamp_unix:
        return CutoffStatus(
            allowed=False,
            reason=f"after_window_deadline:late_by={now_ts - window.deadline_timestamp_unix}s",
            seconds_from_fix=seconds_from_fix,
            window=window,
        )
    return CutoffStatus(
        allowed=True,
        reason="within_fix_window",
        seconds_from_fix=seconds_from_fix,
        window=window,
    )


class DuplicateWindowError(RuntimeError):
    """Raised when a fixing window has already been executed or claimed."""


class WindowStateStore:
    """File-locked durable duplicate-window guard and epoch-chain tracker.

    Never evicts completed windows silently; guarantees at-most-once execution
    for any ``(window_id, base_asset_id, quote_asset_id)`` key and tracks the
    last finalized ``(epoch, packet_hash)`` per asset pair for strict chaining.
    """

    def __init__(self, state_path: Path) -> None:
        self.state_path = Path(state_path)
        self.lock_path = self.state_path.with_suffix(self.state_path.suffix + ".lock")

    @staticmethod
    def pair_key(base_asset_id: str, quote_asset_id: str) -> str:
        return f"{base_asset_id}:{quote_asset_id}"

    @staticmethod
    def window_key(window_id: str, base_asset_id: str, quote_asset_id: str) -> str:
        return f"{window_id}|{base_asset_id}:{quote_asset_id}"

    def _default_state(self) -> dict[str, Any]:
        return {
            "schema": STATE_SCHEMA_V1,
            "windows": {},
            "pair_heads": {},
        }

    def load(self) -> dict[str, Any]:
        if not self.state_path.exists():
            return self._default_state()
        raw = json.loads(self.state_path.read_text(encoding="utf-8"))
        if not isinstance(raw, dict) or raw.get("schema") != STATE_SCHEMA_V1:
            raise ValueError(f"corrupted or unsupported fx-fix state file: {self.state_path}")
        if not isinstance(raw.get("windows"), dict) or not isinstance(raw.get("pair_heads"), dict):
            raise ValueError(f"fx-fix state file missing required maps: {self.state_path}")
        return raw

    def _save_atomic(self, state: dict[str, Any]) -> None:
        self.state_path.parent.mkdir(parents=True, exist_ok=True)
        fd, tmp_name = tempfile.mkstemp(
            dir=str(self.state_path.parent),
            prefix=f".{self.state_path.name}.",
            suffix=".tmp",
        )
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as handle:
                json.dump(state, handle, indent=2, sort_keys=True)
                handle.write("\n")
                handle.flush()
                os.fsync(handle.fileno())
            os.chmod(tmp_name, 0o600)
            os.replace(tmp_name, self.state_path)
        except Exception:
            try:
                os.unlink(tmp_name)
            except OSError:
                pass
            raise

    def get_pair_head(self, base_asset_id: str, quote_asset_id: str) -> dict[str, Any] | None:
        state = self.load()
        return state["pair_heads"].get(self.pair_key(base_asset_id, quote_asset_id))

    def has_window(self, window_id: str, base_asset_id: str, quote_asset_id: str) -> bool:
        state = self.load()
        return self.window_key(window_id, base_asset_id, quote_asset_id) in state["windows"]

    def check_and_record(
        self,
        *,
        window_id: str,
        base_asset_id: str,
        quote_asset_id: str,
        epoch: int,
        packet_hash: str,
        status: str,
        executed_at_utc: str,
        tx_hash: str | None = None,
        dry_run: bool = False,
    ) -> dict[str, Any]:
        """Atomically enforce the duplicate-window guard and record live execution.

        In ``dry_run=True`` mode, this checks for duplicates under the file lock
        without mutating the durable state file.
        """
        self.state_path.parent.mkdir(parents=True, exist_ok=True)
        with open(self.lock_path, "a+", encoding="utf-8") as lock_file:
            fcntl.flock(lock_file.fileno(), fcntl.LOCK_EX)
            try:
                state = self.load()
                wkey = self.window_key(window_id, base_asset_id, quote_asset_id)
                if wkey in state["windows"]:
                    existing = state["windows"][wkey]
                    raise DuplicateWindowError(
                        f"duplicate fix window blocked: {wkey} already recorded "
                        f"(epoch={existing.get('epoch')}, packet_hash={existing.get('packet_hash')})"
                    )
                pkey = self.pair_key(base_asset_id, quote_asset_id)
                head = state["pair_heads"].get(pkey)
                if head is not None:
                    expected_epoch = int(head["epoch"]) + 1
                    if epoch != expected_epoch:
                        raise ValueError(
                            f"epoch sequence mismatch for {pkey}: expected {expected_epoch}, got {epoch}"
                        )
                elif epoch != 1:
                    # Allowed when bootstrapping from an on-chain prior fix; caller verified previous_fix_hash.
                    pass

                record = {
                    "window_id": window_id,
                    "base_asset_id": base_asset_id,
                    "quote_asset_id": quote_asset_id,
                    "epoch": epoch,
                    "packet_hash": packet_hash,
                    "status": status,
                    "executed_at_utc": executed_at_utc,
                    "tx_hash": tx_hash,
                }
                if not dry_run:
                    state["windows"][wkey] = record
                    state["pair_heads"][pkey] = {
                        "epoch": epoch,
                        "packet_hash": packet_hash,
                        "window_id": window_id,
                    }
                    self._save_atomic(state)
                return record
            finally:
                fcntl.flock(lock_file.fileno(), fcntl.LOCK_UN)
