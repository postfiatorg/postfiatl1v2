# Preflight boundary and preparation results

**PASS for offline release inputs; full fleet preflight NOT RUN.**
The exact output is [local-preflight.json](local-preflight.json). No rollout
state, signed manifest, signed backup or fleet receipt was created.

What the supported `scripts/postfiat-safe-rollout preflight` checks, and why it
cannot run before signing, is unchanged from the
[20260921 preflight boundary](../combined-devnet-20260921/PREFLIGHT.md#what-the-supported-command-checks):
it has no offline or unsigned mode, and it reads Vultr, all six hosts and RPC.

## Executed locally on 2026-09-23

```bash
export PYTHONDONTWRITEBYTECODE=1
CACHE="$HOME/.cache/deploy-prep-20260923"
NODE="$HOME/.cache/qualify-fix-20260922/binaries/candidate-1"
CONFIG=deployments/combined-devnet-20260921/rootfs/etc/postfiat/releases/combined-devnet-20260921
sha256sum "$NODE"   # e7bb1afa… (also candidate-2, target-1, target-2)
"$NODE" deployment-validator-units-stage \
  --release-id combined-devnet-20260923 \
  --topology-file "$CONFIG/topology.json" \
  --binary-file "$NODE" \
  --swap-circuit-metadata-file "$CONFIG/swap.metadata.json" \
  --private-egress-circuit-metadata-file "$CONFIG/private-egress.metadata.json" \
  --output-dir "$CACHE/validator-stage"
python3 deployments/combined-devnet-20260923/local-preflight.py \
  --stage "$CACHE/validator-stage" > deployments/combined-devnet-20260923/local-preflight.json
python3 deployments/combined-devnet-20260923/prepare-stage.py \
  --binary "$HOME/.cache/qualify-fix-20260922/target-2/release/postfiat-node" \
  --trusted-publisher-public-file "$HOME/.cache/deploy-prep-20260921/observed/deployment.public.json" \
  --output-dir "$CACHE/regeneration-check/validator-stage"
python3 deployments/combined-devnet-20260923/local-preflight.py \
  --stage "$CACHE/regeneration-check/validator-stage"
```

| Local check | Result |
| --- | --- |
| Qualified executable present, SHA-256 `e7bb1afa…` (4 copies) | PASS; not rebuilt |
| Canonical generator, complete stage schema/order | PASS |
| 33 generated files equal the 20260921 inputs after release-ID substitution | PASS |
| Staged executable equals the qualified SHA-256 | PASS |
| Six RPC/transport pairs and their unit/environment hashes | PASS |
| Six runtime bindings and complete ordered signing bindings | PASS |
| Chain/genesis/protocol and six private-overlay peers | PASS |
| Four embedded circuit artifact hashes and byte lengths | PASS |
| Inventory syntax, unique identities, topology ports | PASS; historical cloud facts only |
| Existing deployment trust-file hash `66304dfc…` | PASS; no public key bytes committed |
| Production allowlist, ten present targets per validator | PASS |
| Production `verify_local_stage`, validators 0–5 | EXPECTED BLOCK: signed manifest missing |
| `prepare-stage.py` regeneration from `target-2` into a second directory | PASS, then the same preflight result |
| `prepare-stage.py` with the 20260921 executable | Refused: `qualified binary SHA-256 mismatch` |
| `observe-fleet.py --help`, `sign-manifest.py --help`, AST parse of four helpers | PASS (no network) |
| `bash -n rollback-one.sh` | PASS |

Output summary: `PASS_LOCAL_INPUTS_ONLY`, `network_calls: 0`,
`rollout_state_created: false`, 33 canonical files, 4 circuit artifacts; each
validator `inputs=PASS`, `allowlisted_present_files=10`,
`manifest_gate=BLOCKED_MISSING_SIGNED_MANIFEST`. The missing eleventh file is
`rootfs/etc/postfiat/releases/combined-devnet-20260923/deployment-manifest.json`.
No dummy file was created.

Validator-0 was read (ten `cat`s under
`/etc/postfiat/releases/a666-source-route-20260907/` and one `systemctl cat` of
four units): every hash and the unit text equal the 2026-09-21 observation
([record](observed/validator-0-recheck.json)). This is not a fleet health claim.

## Blocked on the signature

- The signed `deployment-manifest.json` (path (a) or (b) in [SIGNING.md](SIGNING.md)).
- `local-preflight.py --require-signed` and the six signed runtime checks.
- Full `postfiat-safe-rollout preflight`, which needs the manifest at step 6.
- The signed canary backup (snapshot publisher key) and everything after it.

## Deferred to deployment day

Vultr reconciliation, six-host signer/registry checks, live unit/process
identity, health, convergence, exact remote diffs, the per-host rollback anchors
and signer-safety continuity, and every apply gate. The
[20260921 notes](../combined-devnet-20260921/PREFLIGHT.md#blocked-and-deferred)
on backup verification and pre-activation rollback scope apply unchanged.

No workspace, Orchard/Halo2, rebuild or CI requalification is needed for this
input/documentation packet; see [publication-gates.json](publication-gates.json).
