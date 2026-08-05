# TRUST-ROOT ROTATION CEREMONY PACKET — deployment publisher rotation (2026-08-05)

Status: HELD for Nazgûl ruling. No step herein has been executed. No fire, no
service mutation authorized by this packet.

## 1. Why rotation is required

The accepted deployment publisher is `pf11ea21d97fa9d32eb9d04d060978ff522a58f90e`
(ML-DSA-65, schema `postfiat.deployment_publisher_key.v1`). Its private record
is unrecoverable on this host:

- 1,743 `private_key_hex` candidates scanned (orc_directives, .pft, /tmp):
  zero public-half matches; zero records in the deployment publisher schema.
- Supplementary sweeps empty: archives, local-references, datadump, Documents,
  backups, .stakehub, .pfterminal, shell histories, rollout summaries, release
  pipeline trees (a5, fastswap v1/wan), systemd EnvironmentFiles, mounted
  backup volume `/mnt/HC_Volume_106212907`.
- Forensic finding: signing was ephemeral. `manifest-create-75e18831.json`
  retains signature + public half only; no private-record path was ever
  recorded. Only documentation placeholders (`/secure/deployment.private.json`)
  exist — absent paths.
- Residual unchecked surface: `/home/postfiat/.stakehub/vault.enc` (encrypted,
  5 KB, mode 0600; believed to be the demo wallet vault). One sanctioned probe
  is possible before rotation if ordered (cheap; see §8).

The signed-manifest gate remains mandatory. This packet replaces the trust
anchor; it does not disable, skip, or relax verification.

## 2. Exact trust anchors that change

Live anchors (must change):
1. `StakeHub-repeat-demo/data/wallet-demo-devnet2.json:25` —
   `deployment.trusted_publisher_key_file` → new durable public-key path.
2. The trusted public-key artifact itself → new publisher's
   `deployment.public.json`, stored durably (§4).
3. The deployment manifest → newly created + signed by the new publisher
   (old manifests signed by pf11ea21… become unverifiable under the rotated
   anchor; they are already expired/stale, so nothing live depends on them).

Historical anchors (must NOT change): 82 host-wide `deployment.public.json`
copies (43 pf11ea21…) are release-history artifacts, not live trust state.
Code anchors (`batch_snapshot.rs`, `node_types_snapshot_deployment.rs`,
`group_05.rs`) are key-agnostic — no code change.

## 3. Why this is rotation, not silent gate-weakening

- The gate stays on: every preflight still runs `deployment-manifest-verify`
  and must exit 0.
- The anchor change is explicit, declared, and committed with the new
  publisher identity (address + public-key hash) published in evidence.
- Negative controls (§6) prove the verifier still rejects wrong-publisher,
  tampered, and expired manifests after rotation.
- Custody improves: original key was ephemeral and lost; the new key is
  written mode 0600 by sanctioned tooling into a dedicated mode-700 dir with
  a backup copy (§4 step B).

## 4. Ceremony steps (exact)

All binary invocations use the durable canonical fleet binary
`/home/postfiat/.pft/a666-live-demo/bin/postfiat-node-006167226531582cf81666dded004f26707beedc2ce3fa850caf5b0b82fd22e7`.

A. `install -d -m 700 /home/postfiat/.pft/a666-live-demo/deployment/keys`
B. Generate + back up the new publisher key:
   - `deployment-publisher-key-create --publisher-key-file /home/postfiat/.pft/a666-live-demo/deployment/keys/deployment-publisher-private.json`
     (tool writes 0600 and prints only the public JSON — verified in
     `batch_snapshot.rs:1562-1637`).
   - `deployment-publisher-key-export --publisher-key-file <private> --public-key-file /home/postfiat/.pft/a666-live-demo/deployment/deployment.public.json`
   - Record new publisher address + public-key sha256 in ceremony evidence.
   - Backup: copy the private record to a mode-700 dir on
     `/mnt/HC_Volume_106212907` and write a custody note. NOTE: this volume is
     same-host attached storage; a true off-host copy requires Sauron action
     and is flagged as a follow-up, not assumed.
C. Stage validator units via sanctioned tooling:
   `deployment-validator-units-stage --release-id pftl-escrow-ae3c53c-00616722 --topology-file <durable topology copy> --binary-file <durable binary> --swap-circuit-metadata-file <durable> --private-egress-circuit-metadata-file <durable> --output-dir <new abs dir under .pft>`
   (produces service unit, environment, per-validator bindings, stage-report;
   requires exactly six sorted validators — sixval topology satisfies this).
D. Create + sign the manifest with derived values:
   - `--deployment-id pftl-escrow-ae3c53c-00616722`
   - `--chain-id local-postfiat-lightning-navcoin-demo`
   - `--genesis-hash c9923b5a8326437b0ac4b2fdb6516c13ab3c22f199fea334657ef69f33df31a92e58ac75decd63ca50646ff9f134d3b7`
   - `--git-revision ae3c53c9f07013d2813f3e248eec1d75320d0c03`
   - `--build-profile debug` (fleet attestation, sixval binary-gate evidence)
   - `--build-features privacy,rpc,transport` (canonical prior-manifest policy)
   - `--protocol-version 1`
   - `--rpc-schema postfiat-local-rpc-v1` (canonical prior-manifest policy)
   - `--binary-file` durable fleet binary; unit/env/bindings from step C;
     topology + both circuit metadata from durable copies.
   - Validity window: `valid_from` = ceremony timestamp, `valid_until` = +7
     days, matching the canonical 9500d28e precedent (7d). The two newer
     reference manifests used 30d+5m; 7d is the minimum canonical-compliant
     window and covers the demo with margin. Record exact start/end + basis.
E. Durable install: manifest, public key, and all attested files under
   `/home/postfiat/.pft/a666-live-demo/deployment/` (mode-700 root; private
   key 0600; public artifacts 0644). Profile references durable paths only —
   never /tmp.
F. Profile update (StakeHub-repeat-demo): `deployment.*` paths → durable
   copies; `build.node_bin` + `node_bin_sha256` re-applied (fleet binary
   `00616722…`); recompute canonical identity programmatically; commit.
G. §9.1 atomic restage → one full devnet2 preflight (exit 0 required) →
   fresh fire preflight → Phase B legs 1→5 under the standing Sauron ruling.

## 5. Rollback

- Profile: `git revert --no-edit` the profile commit (prior committed state
  restores the old trusted-key path + old pins).
- Services: restore to committed snapshot `fb022ba` (8787 unit file install,
  bfinal transient recreation with prior hash, kill 18793 process group).
- Rotation adds no on-chain or irreversible state; the trust anchor is local
  configuration. Old publisher artifacts remain untouched for history.

## 6. Tests and negative controls

Positive: production verifier exit 0 on the new manifest with the new trusted
key; full devnet2 preflight exit 0.
Negative controls (all must FAIL closed):
1. Verify new manifest against the OLD pf11ea21 public key → must fail
   (proves anchor moved).
2. Tamper one attested hash → must fail.
3. `valid_until` in the past → must fail (expiry).
Regression suite:
- `cargo test -p postfiat-node signed_deployment_manifest_rejects_tampering_expiry_and_wrong_publisher -- --exact`
- `cargo test -p postfiat-node deployment_validator_unit_stage_is_canonical_and_non_overwriting -- --exact`
- `python3 -m pytest tests/test_wallet_demo.py -q -k deployment`

## 7. Dual-sign / continuity options considered

The verifier is single-key only (`node_types_snapshot_deployment.rs:152-163`;
equality check `batch_snapshot.rs:2083-2090`). No multi-key, overlap, or
previous-key mechanism exists. Options: (a) hard rotation — CHOSEN, safe
because every old-anchor manifest is already expired and nothing live
references them; (b) verifier code change for key lists — rejected (touches
verification code mid-campaign, out of scope, itself a gate change).

## 8. Duration and residual items

- Ceremony (A–E): ~20 min. Restage + preflights: ~30 min. Fire: within
  packet timeouts.
- Residual unchecked surface: `/home/postfiat/.stakehub/vault.enc` — one
  sanctioned probe for a deployment-publisher record is possible pre-rotation
  if ordered; if it yields the original key, this packet is void and the
  original anchor is retained.
- True off-host key custody requires Sauron action (§4 step B note).

Evidence inputs: /tmp/ghash-keyhunt2/, /tmp/krimp-keyhunt2/,
/tmp/snaga-rotation/, /tmp/ghash-manifest/, /tmp/krimp-manifest/.
