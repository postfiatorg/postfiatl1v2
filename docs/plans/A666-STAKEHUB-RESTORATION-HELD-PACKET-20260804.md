# A666 StakeHub Restoration HELD Packet — 2026-08-04

**Status: HELD — requires Sauron's explicit GO before any command in section 4.**

## 1. Safety boundary

This packet restores StakeHub service roles only. It does not delete,
decommission, uninstall, migrate, empty, rewrite, or inspect StakeHub funds,
balances, keys, software, configuration values, vaults, or application data.
The permitted execution, if approved, is limited to systemd manager environment
injection of one public profile hash, starting named user services, and creating
one transient dashboard candidate from the canonical command recorded by
systemd.

Current facts:

- `stakehub-private-swap-dashboard.service` is active.
- Nothing was deleted.
- Funds, balances, keys, vaults, and application data remain untouched.
- The failed R4 restart proof is
  `docs/evidence/a666-public-reserve-product-20260803/browser/r4-pass1/stakehub-restart-proof.json`
  at commit `df7dd36`.

## 2. Read-only diagnosis

The mode-600 R4 inventory recorded five active units. Read-only
`systemctl --user show`, `list-unit-files`, and redacted journal tails prove:

| Inventoried unit | Root cause |
|---|---|
| `stakehub-pfusdc-tier4-wallet-agent-20260718.service` | `LoadState=not-found`; its transient file under `/run/user/1000/systemd/transient/` disappeared when stopped. |
| `stakehub-private-swap-asset-orchard-candidate.service` | `LoadState=not-found`; it was transient. The journal preserves its public command shape and loopback port 18792. |
| `stakehub-private-swap-dashboard-bfinal.service` | `LoadState=not-found`; it was transient. The journal preserves its public command shape and loopback port 8788. |
| `stakehub-private-swap-asset-orchard.service` | Persistent and enabled, but startup exits because the required public `STAKEHUB_PRODUCT_PROFILE_SHA256` environment field is absent. |
| `stakehub-private-swap-dashboard.service` | Persistent, enabled, and active on port 8787. |

Canonical persistent units still present:

- `stakehub-private-swap-dashboard.service`
- `stakehub-private-swap-asset-orchard.service`
- `stakehub-private-swap-wallet-agent.service`
- `stakehub-pfusdc-wallet-agent.service`

Canonical source evidence:

- `/home/postfiat/repos/orc_directives/solid_e2e_stage3_supervised_services_20260710/SUPERVISED-WALLET-SERVICES-PROOF-20260710.md`
- `/home/postfiat/repos/orc_directives/solid_e2e_stage3_supervised_services_20260710/supervised-units.json`
- `/home/postfiat/repos/orc_directives/solid_e2e_stage3_supervised_services_20260710/private-swap.env`

The historical proof directs operators to use the installed user-systemd units,
reload the user manager, and enable/start one unit at a time after readiness
verification. The public profile hash already exists in
`supervised-units.json`; this packet never prints it.

## 3. Restoration choice and declared deviation

The exact transient pfUSDC agent launch command was never persisted, and its
argv-free R4 inventory cannot recreate it. Recreating all five historical names
is therefore impossible from authoritative evidence.

Recommended functional restoration uses the four canonical persistent service
roles plus the journal-proven bfinal dashboard candidate on port 8788. Expected
active count remains five:

1. canonical dashboard on 8787;
2. canonical Asset-Orchard on 18792;
3. canonical private-swap wallet agent;
4. canonical pfUSDC wallet agent;
5. transient bfinal dashboard candidate on 8788.

Declared deviation: two historical transient names are replaced by their
canonical persistent service roles, and the duplicate transient
Asset-Orchard candidate is omitted to avoid a port-18792 collision.

## 4. HELD execution recipe

**Do not execute without Sauron's explicit GO.**

Run from a shell owned by user `postfiat`:

```bash
set -euo pipefail

UNIT_SOURCE=/home/postfiat/repos/orc_directives/solid_e2e_stage3_supervised_services_20260710
PROFILE_SHA="$(
  jq -er '.profile_sha256 | select(type == "string" and test("^[0-9a-f]{64}$"))'     "$UNIT_SOURCE/supervised-units.json"
)"

# Touches only the user-systemd manager environment with a public identity hash.
systemctl --user set-environment STAKEHUB_PRODUCT_PROFILE_SHA256="$PROFILE_SHA"

# Re-read existing unit metadata; no unit file or application data is modified.
systemctl --user daemon-reload

# Start canonical roles one at a time so one failure cannot suppress the rest.
systemctl --user start stakehub-private-swap-dashboard.service
systemctl --user start stakehub-private-swap-asset-orchard.service
systemctl --user start stakehub-private-swap-wallet-agent.service
systemctl --user start stakehub-pfusdc-wallet-agent.service

# Recreate only the journal-proven bfinal dashboard candidate on distinct port 8788.
systemd-run --user   --unit=stakehub-private-swap-dashboard-bfinal   --description='StakeHub private swap dashboard bfinal'   --working-directory=/home/postfiat/repos/StakeHub-repeat-demo   --property="EnvironmentFile=$UNIT_SOURCE/private-swap.env"   /home/postfiat/repos/StakeHub/.venv/bin/python   -m stakehub.cli dashboard --port 8788

# Remove the temporary manager value after child environments are established.
systemctl --user unset-environment STAKEHUB_PRODUCT_PROFILE_SHA256
```

## 5. Required post-start checks

```bash
set -euo pipefail
units=(
  stakehub-private-swap-dashboard.service
  stakehub-private-swap-asset-orchard.service
  stakehub-private-swap-wallet-agent.service
  stakehub-pfusdc-wallet-agent.service
  stakehub-private-swap-dashboard-bfinal.service
)
for unit in "${units[@]}"; do
  test "$(systemctl --user is-active "$unit")" = active
done
test "${#units[@]}" -eq 5
curl -fsS http://127.0.0.1:18792/asset-orchard/readiness   | jq -e '.ready == true' >/dev/null
```

Evidence must record unit names, active booleans, expected/observed count, and
Asset-Orchard readiness only. It must exclude process argv, environment values,
authentication material, and application data.

Any command failure, count mismatch, readiness failure, unexpected port owner,
or request for application/config changes: **STOP-no-retry and return HELD.**

## 6. Touch ledger

| Command class | What it touches | Why safe under approved scope |
|---|---|---|
| `systemctl set/unset-environment` | User-systemd manager environment, public profile hash only | Supplies a public product identity already committed in the canonical supervised-unit manifest. |
| `systemctl daemon-reload` | User-systemd metadata cache | Reads existing unit files; writes no StakeHub application state. |
| `systemctl start` | Existing named user services | The standing constraint explicitly permits unit start/stop. |
| `systemd-run` bfinal | One transient user service on loopback port 8788 | Reuses the exact journal-proven command; distinct from the canonical dashboard port. |
| Readiness checks | User-systemd active state and public loopback readiness | Reads service health only; no authentication or application data. |
