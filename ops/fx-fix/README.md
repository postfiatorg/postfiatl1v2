# Scheduled London-Style FX Fix Executor (`ops/fx-fix/`)

Deterministic scheduler, validator, signer boundary, and settlement batch runner for `FxFixPacketV1` (`crates/types/src/fx_fix_types.rs`).

## Features

- **Exact L1 wire parity:** Computes `FxFixPacketV1::canonical_hash()` (`postfiat.fx_fix.packet_hash.v1` + `0x00` + canonical field preimage via SHA3-384) and `fx_fix_reservation_id()` (`postfiat.fx_fix.reservation_id.v1`), cross-verified against `postfiat_types::FxFixPacketV1` in Rust.
- **Pillar 7 NAVCoin fee cap enforcement:** Rejects any `fee_bps > 100` (protocol 1% cap) or `fee_bps > nav_fix_fee_cap_bps` on NAV-tracked pairs.
- **DST-aware London fixing schedule:** Resolves `Europe/London` (GMT/BST) fixing windows, skips weekends and configured holidays, enforces pre-fix order cutoff (`cutoff_seconds_before_fix`) and post-fix deadline (`max_lateness_seconds`).
- **Fail-closed duplicate-window guard:** Uses an `flock`-protected atomic JSON state store (`window-state.json`) keyed by `(window_id, base_asset_id, quote_asset_id)` to guarantee at-most-once execution and monotonic `epoch` / `previous_fix_hash` chaining.
- **Dry-run mode & isolated signing boundary:** `--dry-run` builds, validates, and signs without mutating chain or window state; live runs delegate signing to an external signer command (`--signer-cmd`) so key material never enters this process.

## Usage

```bash
# Check whether the current UTC time falls inside the 16:00 Europe/London window
PYTHONPATH=ops/fx-fix python3 -m fx_fix_executor.cli plan-window \
  --config ops/fx-fix/config.example.json \
  --now-utc 2026-11-27T16:00:10Z

# Dry-run one London fixing batch
PYTHONPATH=ops/fx-fix python3 -m fx_fix_executor.cli run \
  --config ops/fx-fix/config.example.json \
  --state-file /tmp/fx-fix-state.json \
  --observations /path/to/observations.json \
  --block-height 250000 \
  --now-utc 2026-11-27T16:00:10Z \
  --dry-run
```

## Systemd deployment

Install `systemd/postfiat-fx-fix.service` and `systemd/postfiat-fx-fix.timer` to `/etc/systemd/system/`, place runtime config in `/etc/postfiat/fx-fix.json` (`0600`), and enable the timer:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now postfiat-fx-fix.timer
sudo systemctl list-timers postfiat-fx-fix.timer
```

## Tests

```bash
# Python unit tests
PYTHONPATH=ops/fx-fix pytest ops/fx-fix/tests/

# Cross-language Rust parity test against crates/types::FxFixPacketV1
cargo test --manifest-path ops/fx-fix/rust-verify/Cargo.toml
```
