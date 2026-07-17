# WAN Devnet Structural Fix

Status: structural fixes implemented; mixed Hetzner/Vultr topology superseded
Audience: PFTL operator, protocol engineer, validator operator
Date: 2026-06-21

This runbook describes the structural fix for the WAN devnet split observed
after the live a651 <-> pfUSDC round trip. It separates immediate recovery from
the permanent guardrails required before more live-value demos.

Topology update: the mixed Hetzner/Vultr fleet documented in the historical
incident evidence below is no longer the active testnet topology. The active
testnet is all Vultr. Use
`docs/runbooks/wan-devnet-full-live-end-to-end-run.md` and
`$POSTFIAT_STATE/live-e2e-20260621T061254Z/all-vultr-remote-topology.json`
for current validator addresses. Do not use Hetzner hosts for new live runs.

The issue is not that the bridge flow itself failed. The round trip reached a
valid quorum and completed the economic proof, but the public six-validator
fleet did not remain synchronized. Future live-value runs must fail closed
before that can happen.

## Executive Summary

The WAN devnet failure was an operator-safety and replay-hardening failure, not
an a651, pfUSDC, bridge, or shielded-swap economic failure.

The live run advanced through quorum while two public validators were not fully
caught up. That was possible because the runner allowed degraded peer handling,
accepted quorum-only progress as operational success, and confused a local
validator data directory with a public validator identity. Once validator-4 was
skipped after quorum, it correctly refused later proposals because its local
parent state no longer matched. Repair was then blocked by historical archive
replay incompatibility.

The structural fix is:

1. live-value runs must require all active public validators, not only quorum;
2. public endpoint evidence must be the source of truth for final summaries;
3. local operator state must never impersonate a public validator identity;
4. archive replay must be version-aware so old history can be verified and
   repaired safely;
5. catch-up and snapshot repair must preflight before services are stopped or
   validator state is mutated.

The code guardrails and local replay/snapshot repair gates below are
implemented and verified against the saved WAN archive. The two stale public
validators have been repaired one at a time with verified catch-up, and all six
public RPC endpoints now converge to the same height, tip hash, and state root.

No new live-value Arbitrum round trip or shielded swap demo should use the old
degraded runner settings. Future runs must use the repaired runner guardrails
and must treat public endpoint convergence, not quorum-only progress, as the
success condition.

## Current Known State

The active state is the all-Vultr topology described in
`docs/runbooks/wan-devnet-full-live-end-to-end-run.md`. The former mixed
Hetzner/Vultr evidence below is historical incident evidence only. It must not
be used as a current topology, fallback plan, or operator command source.

Fresh RPC status after the original mixed-fleet incident, before repair:

```text
validator-0 height 119 root 800f978e...
validator-1 height 119 root 800f978e...
validator-2 height 119 root 800f978e...
validator-3 height 105 root 2832df6f...
validator-4 height 114 root 027c0a0d...
validator-5 height 119 root 800f978e...
```

At incident time, the devnet was degraded, not erased: four public validators
agreed at height 119, and two public validators were stale. Those two stale
validators have since been repaired; the final convergence table below is the
current source of truth.

Historical RPC status after repair on the retired mixed fleet:

```text
validator-0 198.51.100.12:27650 height=119 root=800f978e557056e6a70b0e5748abfc28f18d0eef54e232032687510c1759f8a555155b4039100df818f575f08f71aeb5 tip=f6f96a0f006fee0f668280aa40bca2c142a42976c0f849316f2666a6f5ace118c554e92f155c5286ff8d09fa27b67a70
validator-1 198.51.100.10:27651 height=119 root=800f978e557056e6a70b0e5748abfc28f18d0eef54e232032687510c1759f8a555155b4039100df818f575f08f71aeb5 tip=f6f96a0f006fee0f668280aa40bca2c142a42976c0f849316f2666a6f5ace118c554e92f155c5286ff8d09fa27b67a70
validator-2 198.51.100.11:27652 height=119 root=800f978e557056e6a70b0e5748abfc28f18d0eef54e232032687510c1759f8a555155b4039100df818f575f08f71aeb5 tip=f6f96a0f006fee0f668280aa40bca2c142a42976c0f849316f2666a6f5ace118c554e92f155c5286ff8d09fa27b67a70
validator-3 192.0.2.13:27653 height=119 root=800f978e557056e6a70b0e5748abfc28f18d0eef54e232032687510c1759f8a555155b4039100df818f575f08f71aeb5 tip=f6f96a0f006fee0f668280aa40bca2c142a42976c0f849316f2666a6f5ace118c554e92f155c5286ff8d09fa27b67a70
validator-4 192.0.2.14:27654 height=119 root=800f978e557056e6a70b0e5748abfc28f18d0eef54e232032687510c1759f8a555155b4039100df818f575f08f71aeb5 tip=f6f96a0f006fee0f668280aa40bca2c142a42976c0f849316f2666a6f5ace118c554e92f155c5286ff8d09fa27b67a70
validator-5 192.0.2.15:27655 height=119 root=800f978e557056e6a70b0e5748abfc28f18d0eef54e232032687510c1759f8a555155b4039100df818f575f08f71aeb5 tip=f6f96a0f006fee0f668280aa40bca2c142a42976c0f849316f2666a6f5ace118c554e92f155c5286ff8d09fa27b67a70
unique_height_root_tip_count=1
```

That retired mixed fleet was no longer split at the public RPC layer. This repair did not
replace `/usr/local/bin/postfiat-node` on the validator hosts. The catch-up was
performed with the staged repaired binary at `/tmp/postfiat-node-structural-fix`
with SHA3-384:

```text
0a063e2a9bd71b73b0d721635386a8c673ced0c8f9ea7095ceba20151dd1f44f62959d404cc93ba9638545f843983095
```

If the operator wants every validator service binary to include the archive
replay compatibility code, that is a separate rolling binary deployment. It was
not needed to repair the stale public state roots.

Current active all-Vultr state after the replacement plan:

```text
validator-0 192.0.2.10:27650 height=146 root=cdcf58d68aa4cf57912de67a3da0a1994c0a328a5537672de3551eaa8e1805198dc3a62e774c29f70a098a0480e48b28
validator-1 192.0.2.11:27651 height=146 root=cdcf58d68aa4cf57912de67a3da0a1994c0a328a5537672de3551eaa8e1805198dc3a62e774c29f70a098a0480e48b28
validator-2 192.0.2.12:27652 height=146 root=cdcf58d68aa4cf57912de67a3da0a1994c0a328a5537672de3551eaa8e1805198dc3a62e774c29f70a098a0480e48b28
validator-3 192.0.2.13:27653 height=146 root=cdcf58d68aa4cf57912de67a3da0a1994c0a328a5537672de3551eaa8e1805198dc3a62e774c29f70a098a0480e48b28
validator-4 192.0.2.14:27654 height=146 root=cdcf58d68aa4cf57912de67a3da0a1994c0a328a5537672de3551eaa8e1805198dc3a62e774c29f70a098a0480e48b28
validator-5 192.0.2.15:27655 height=146 root=cdcf58d68aa4cf57912de67a3da0a1994c0a328a5537672de3551eaa8e1805198dc3a62e774c29f70a098a0480e48b28
unique_height_root_tip_count=1
```

The height-146 state is after the all-Vultr real Arbitrum USDC round trip in
`$POSTFIAT_STATE/live-e2e-20260621T061254Z/roundtrip-live-20260621T063208Z`.
That run completed with `final_summary_ok=true`, matched the expected NAV
money-in and money-out deltas, and settled the PFTL redemption accounting.

## Implementation Status

Implemented locally on 2026-06-20:

- Strict live-value guardrails reject degraded finality flags for
  `nav-roundtrip-live-demo`.
- Fleet preflight records separate `operator_local_state` from
  `public_validator_states` and rejects local/public validator identity
  collisions.
- Final summaries require public validator endpoint evidence; a stale or
  missing public endpoint cannot be reported as green.
- `rpc-catch-up` performs cheap preflight validation before mutating work
  directories or validator state.
- Archive replay is version-aware for the saved WAN history:
  - block-9 legacy transparent batch self-id compatibility;
  - legacy NAV profile id/signing preimages;
  - legacy NAV subscription source-root compatibility;
  - exact legacy domainless withdrawal packet emission only for burn batches
    at heights 46 and 60;
  - domain-bound withdrawal packets for later burns at heights 89, 104, and
    118;
  - exact legacy AssetOrchard swap proof acceptance only for archived swap
    actions at heights 76, 83, and 84.

Local verification evidence:

```text
cargo build --release -p postfiat-node
  PASS, with existing rpc_cli.rs warnings only

target/release/postfiat-node verify-blocks \
  --data-dir $POSTFIAT_STATE/shielded-pfusdc-wan-20260620T/data
  PASS: verified=true, block_count=119,
        tip_hash=f6f96a0f006fee0f668280aa40bca2c142a42976c0f849316f2666a6f5ace118c554e92f155c5286ff8d09fa27b67a70,
        state_root=800f978e557056e6a70b0e5748abfc28f18d0eef54e232032687510c1759f8a555155b4039100df818f575f08f71aeb5

target/release/postfiat-node snapshot-export \
  --data-dir $POSTFIAT_STATE/shielded-pfusdc-wan-20260620T/data \
  --snapshot-dir /tmp/postfiat-wan-snapshot-check
  PASS: block_height=119,
        state_root=800f978e557056e6a70b0e5748abfc28f18d0eef54e232032687510c1759f8a555155b4039100df818f575f08f71aeb5

cargo test -p postfiat-node nav_roundtrip -- --nocapture
  PASS: 15 tests

cargo test -p postfiat-node archived_transparent_replay_accepts_wan_devnet_legacy_batch_id_only -- --nocapture
  PASS: 1 test

cargo test -p postfiat-node rpc_catch_up_rejects_zero_max_blocks_before_work_dir_mutation -- --nocapture
  PASS: 1 test

cargo test -p postfiat-node wan_devnet_legacy -- --nocapture
  PASS: 3 tests

cargo fmt --check
  PASS
```

Fleet repair evidence:

```text
validator-3 copy preflight:
  source validator-0 height=119 root=800f978e557056e6a70b0e5748abfc28f18d0eef54e232032687510c1759f8a555155b4039100df818f575f08f71aeb5
  local before height=105 root=2832df6fd959c7c1f1f890f169bf8035e9ed28f9e1802e310db3fdf23010cc81513380b8f69b7bcac0ea379c6e3b826c
  copy after height=119 root=800f978e557056e6a70b0e5748abfc28f18d0eef54e232032687510c1759f8a555155b4039100df818f575f08f71aeb5
  applied heights=106..119
  result=caught_up

validator-4 copy preflight:
  source validator-0 height=119 root=800f978e557056e6a70b0e5748abfc28f18d0eef54e232032687510c1759f8a555155b4039100df818f575f08f71aeb5
  local before height=114 root=027c0a0dd195253495430c19ae46540762d2c1fc900b8c26ac688cb69ef1eb45f1f21dd044d4f25da838e4cc9e1ddf5e
  copy after height=119 root=800f978e557056e6a70b0e5748abfc28f18d0eef54e232032687510c1759f8a555155b4039100df818f575f08f71aeb5
  applied heights=115..119
  result=caught_up

validator-3 live repair:
  service stop scope=postfiat-validator-3.service, postfiat-validator-3-rpc.service
  backup=/root/validator-3-pre-structural-repair-20260621T010959Z.tgz
  source validator-0 height=119 root=800f978e557056e6a70b0e5748abfc28f18d0eef54e232032687510c1759f8a555155b4039100df818f575f08f71aeb5
  before height=105 root=2832df6fd959c7c1f1f890f169bf8035e9ed28f9e1802e310db3fdf23010cc81513380b8f69b7bcac0ea379c6e3b826c
  after height=119 root=800f978e557056e6a70b0e5748abfc28f18d0eef54e232032687510c1759f8a555155b4039100df818f575f08f71aeb5
  tip=f6f96a0f006fee0f668280aa40bca2c142a42976c0f849316f2666a6f5ace118c554e92f155c5286ff8d09fa27b67a70
  applied_count=14
  service restart=active/active

validator-4 live repair:
  service stop scope=postfiat-validator-4.service, postfiat-validator-4-rpc.service
  backup=/root/validator-4-pre-structural-repair-20260621T011818Z.tgz
  source validator-0 height=119 root=800f978e557056e6a70b0e5748abfc28f18d0eef54e232032687510c1759f8a555155b4039100df818f575f08f71aeb5
  before height=114 root=027c0a0dd195253495430c19ae46540762d2c1fc900b8c26ac688cb69ef1eb45f1f21dd044d4f25da838e4cc9e1ddf5e
  after height=119 root=800f978e557056e6a70b0e5748abfc28f18d0eef54e232032687510c1759f8a555155b4039100df818f575f08f71aeb5
  tip=f6f96a0f006fee0f668280aa40bca2c142a42976c0f849316f2666a6f5ace118c554e92f155c5286ff8d09fa27b67a70
  applied_count=5
  service restart=active/active

final public RPC convergence:
  validators=0..5
  height=119
  root=800f978e557056e6a70b0e5748abfc28f18d0eef54e232032687510c1759f8a555155b4039100df818f575f08f71aeb5
  tip=f6f96a0f006fee0f668280aa40bca2c142a42976c0f849316f2666a6f5ace118c554e92f155c5286ff8d09fa27b67a70
  unique_height_root_tip_count=1
```

Still pending before the next live-value demo:

- Use the repaired live-value runner guardrails, not the old degraded runner
  settings.
- Write the run manifest before moving funds.
- Treat all-six public endpoint convergence as the final success condition.

## Root Cause

The root cause is a chain of safety gaps:

1. The live runner used a local data dir whose `node_id` was `validator-3`.
   The topology also contained a public `validator-3` at `192.0.2.13`. The
   runner treated the local state as validator-3, so it did not send certified
   blocks to the real public validator-3. Public validator-3 stayed at height
   105 while the local validator-3 clone advanced to height 119.

2. The run allowed peer failures. The finality reports show
   `allow_peer_failures: true` and only four remote votes required. That let
   the demo continue after public peers were unresolved.

3. Validator-4 was skipped after early quorum. Around height 115, the reports
   show `skipped_certified_send_targets: ["validator-4"]`. Validator-4 remained
   at height 114 and then correctly rejected later vote requests because the
   proposals no longer matched its local parent state:

   ```text
   transport block vote signing failed:
   block proposal does not match local batch and state
   ```

4. The repair path was blocked by historical archive verification.
   `verify-blocks`, `snapshot-export`, and `rpc-catch-up` failed at block 9:

   ```text
   block 9 archived transparent payload invalid:
   batch id mismatch expected ad8f94... got ffcd76...
   ```

   The stored historical batch self-identifies as `ffcd...`, while the current
   verifier recomputes a different canonical transparent batch id. This is a
   replay-compatibility bug for historical transparent batches, and it
   prevented safe catch-up. This is now fixed locally with chain-specific,
   block-specific archive replay compatibility.

5. The success summary mixed local and public validator state. It listed local
   validator-3 at height 119 as if that represented the public validator-3
   process. Live summaries must distinguish local operator state from public
   validator endpoint state.

## Required Invariants

These invariants must hold before any further live-value run:

- One active validator identity maps to one public validator endpoint. A local
  data dir may not silently replace a public `node_id`.
- Every active public validator must report the expected `node_id`, chain id,
  genesis hash, height, tip hash, and state root before a live run starts.
- A live-value run may not use quorum-only success as final success. The final
  summary is green only if every active public validator reaches the final
  expected height/root.
- Certified sends must reach every active public validator in live-value mode.
  If a validator is unresolved, stop before the next state transition.
- Catch-up, snapshot, and replay verification must pass on the chain history
  that the devnet actually produced.
- Recovery commands must fail before stopping services if local arguments,
  source RPC, work directories, or archive verification are invalid.

## Immediate Stabilization

This section is complete as of 2026-06-21. Do not run more live-value demos
unless the strict public-fleet guardrails remain enabled.

1. Preserve the incident artifacts.

   - Keep `$POSTFIAT_STATE/nav-roundtrip-speedrun-full-20260620T201355Z`.
   - Keep `$POSTFIAT_STATE/shielded-pfusdc-wan-20260620T/data`.
   - Do not rewrite stale validator data dirs before taking backups.

2. Confirm public endpoint state.

   Query all six public RPC endpoints and write a short status artifact with
   node id, height, tip hash, and state root. Do not rely on local run summary
   files for public validator health.

3. Fix historical archive verification in code. Status: implemented locally.

   The verifier needs version-aware transparent batch id handling. For legacy
   transparent blocks, validation must prove:

   - the block header batch id equals the archived batch id;
   - the archived payload self-id equals the block header batch id;
   - the archived payload hash matches the archive entry;
   - replay from that archived payload produces the recorded receipts and state
     transition;
   - tampering with the archived payload still fails.

   It must not reject a historical block solely because the current transparent
   batch reference algorithm recomputes a newer id for an older payload.

4. Prove the replay fix locally before touching validators. Status: complete
   for the saved WAN archive.

   Required checks:

   ```bash
   cargo test -p postfiat-node wan_devnet_legacy -- --nocapture
   target/release/postfiat-node verify-blocks \
     --data-dir $POSTFIAT_STATE/shielded-pfusdc-wan-20260620T/data
   target/release/postfiat-node snapshot-export \
     --data-dir $POSTFIAT_STATE/shielded-pfusdc-wan-20260620T/data \
     --snapshot-dir /tmp/postfiat-wan-snapshot-check
   ```

5. Repair stale validators one at a time. Status: complete.

   For validator-3, then validator-4:

   - back up the data dir;
   - run a catch-up dry run from a known-good source;
   - stop only that validator service;
   - run verified catch-up or verified snapshot import;
   - restart that validator;
   - confirm all six public validators converge to the same height/root.

   Do not raw-copy mutable state into a live validator without verification.
   The completed repair used verified `rpc-catch-up`, not raw state copy.

## Structural Fix Plan

### P0: Strict Live-Fleet Mode

Add a strict mode for live-value runner paths such as
`nav-roundtrip-live-demo`.

Default behavior for live-value mode:

- `--allow-peer-failures` is rejected unless an explicit
  `--degraded-topology-ack FILE` is supplied.
- Any unresolved active validator makes the stage fail.
- Any skipped certified send makes the stage fail.
- `quorum_early_full_propagation` must be true or the stage must explicitly
  continue propagation after quorum before reporting success.
- The final run summary is green only if every public validator endpoint reports
  the final expected height/root.

Quorum-only success is still useful for adversarial or outage tests, but it
must be labeled as degraded and must not be used as the success condition for a
live-value bridge/NAV demo.

### P0: Public Validator Preflight

Before a live-value run starts, poll every topology peer by public RPC and
transport identity.

Required checks:

- topology `node_id` equals RPC `node_id`;
- chain id and genesis hash match the run manifest;
- service is active and responding;
- height, block hash, and state root match the current fleet tip;
- the local data dir `node_id` does not duplicate a public validator unless
  the run is explicitly executing on that validator host.

If the operator machine needs local state for quoting or artifact generation,
that local state must be labeled `operator_local_state`, not a validator state.

### P0: Identity Collision Guard

Add a hard guard to the certified-runner stack:

```text
if local_node_id is present in topology
and local data dir is not the matching public validator endpoint
and live_value_mode is true:
    fail before signing or applying anything
```

The run that just happened would have failed at this guard because the local
data dir was `validator-3` while a public `validator-3` endpoint also existed.

Longer term, live demos should use one of these patterns:

- drive finality through the actual remote validator service; or
- use an explicit non-validator operator client that does not claim a validator
  identity and cannot be counted as a validator state.

### P0: Final Summary Must Use Public Endpoint Evidence

Update run summaries to contain two separate sections:

```json
{
  "operator_local_state": {},
  "public_validator_states": []
}
```

The `final_summary_ok` field must be false if any active public validator is
missing, stale, divergent, or unreachable. A local validator clone cannot fill
the slot for a public validator endpoint.

### P0: Safe Catch-Up Preflight

`rpc-catch-up` and snapshot repair commands must perform all cheap validation
before stopping services or mutating state:

- validate `--max-blocks`;
- validate work-dir writability;
- query source status;
- check source archive availability;
- verify source replay compatibility for the requested range;
- write a dry-run report.

Only after that preflight passes should an operator stop a validator and apply
state. The earlier failed catch-up attempts stopped services before discovering
argument and work-dir errors; that should not be possible.

### P1: Versioned Archive Verification

Introduce an explicit archive/batch-id compatibility layer.

The verifier should know which transparent batch id scheme applies to a
historical block. Options:

- infer legacy scheme by block height and chain id for controlled devnet
  history;
- persist a `batch_id_scheme` or `archive_schema` field in new archive entries;
- maintain a chain-specific migration manifest for old blocks.

The rule is not "accept anything old." The rule is "verify old blocks according
to the canonical rules that were active when those blocks were produced, then
verify replayed state." Tampered old payloads must still fail.

Required regression tests:

- legacy transparent block archive verifies;
- legacy payload tampering fails;
- current transparent batch id mismatch fails;
- `snapshot-export` works on a chain with legacy transparent batches;
- `rpc-catch-up` works from a source with legacy transparent batches.

### P1: Certified-Send Completion Semantics

The peer-certified round should distinguish:

- quorum reached;
- certificate built;
- certified batch propagated to every active validator;
- every active validator acknowledged the final height/root.

For live-value mode, success means all four happened. If quorum is reached but
one validator did not receive the certified batch, the stage must stop and
report a degraded fleet, not advance to the next transaction.

### P1: Fleet Health Gate in CI

Add a simulated six-validator test battery:

- duplicate local/public validator identity is rejected;
- one unresolved validator makes live-value strict mode fail;
- skipped certified send makes live-value strict mode fail;
- final summary cannot be green if a public endpoint is stale;
- quorum-only degraded mode remains available for explicit outage drills;
- catch-up from a legacy archive fixture succeeds after the archive fix.

### P2: Operator Run Manifest

Every live-value run should write and consume a manifest with:

- run id;
- topology file hash;
- public endpoints;
- expected active validator count and quorum;
- local data dir identity;
- binary hash;
- git commit;
- dirty-worktree flag;
- contract addresses;
- source-chain id;
- asset ids;
- exact finality flags.

The manifest should be written before funds move. If the manifest contains a
duplicate validator identity, stale fleet state, dirty consensus binary without
operator acknowledgement, or degraded finality flags, the run should stop.

## Validator Repair Protocol

Use this only after the archive verification fix is proven locally.

1. Pick a source validator at the agreed height/root, currently one of
   validator-0, validator-1, validator-2, or validator-5.

2. Repair one stale validator at a time.

3. Back up the stale data dir:

   ```bash
   sudo systemctl stop postfiat-validator-N.service postfiat-validator-N-rpc.service
   sudo tar -C /var/lib/postfiat -czf /root/validator-N-pre-repair.tgz validator-N
   ```

4. Run verified catch-up or verified snapshot import.

5. Restart services:

   ```bash
   sudo systemctl start postfiat-validator-N.service postfiat-validator-N-rpc.service
   ```

6. Poll all six public endpoints. Proceed to the next validator only if all
   non-target validators remain on the same root and the repaired validator
   rejoins that root.

7. Stop immediately if a validator rejoins at a different root.

Authentication hygiene:

- The active validator set is all Vultr.
- All active validators use root SSH key auth with `~/.ssh/id_ed25519`.
- Do not use Hetzner hosts for new live runs.
- Do not use `sshpass` or put plaintext passwords on the command line.

## Definition of Done

The local code/replay portion is complete when all of the following are true:

- `verify-blocks` passes on the current WAN history.
- `snapshot-export` works on the current WAN history.
- The live-value runner refuses to start with a duplicate local/public
  validator identity.
- The live-value runner refuses to proceed when any active validator is
  unresolved or skipped.
- The final summary uses public endpoint evidence and cannot be green with a
  stale public validator.
- A six-validator simulation covers duplicate identity, skipped certified send,
  stale endpoint, and catch-up from legacy archive history.

The fleet repair portion is complete only when all of the following are true:

- A verified catch-up or snapshot import has been run for validator-3 and
  validator-4 one at a time.
- `rpc-catch-up` or the selected snapshot-import path has been proven against
  the current WAN history before each service stop.
- Public validators 0 through 5 all report the same height, block hash, and
  state root.

This definition is now met for the height-119 incident repair. Only after the
strict runner guardrails and manifest discipline are used should another live
Arbitrum USDC round trip or shielded swap demo run on WAN devnet.
