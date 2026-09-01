# Public Operator Runbook

Status: Phase D2 deliverable of the
[public-testnet path milestone](../plans/active/l1v2-public-testnet-path-milestone.md)
Date: 2026-09-01
Audience: a prospective validator operator with no prior relationship to this
project — "a stranger" — preparing to run an l1v2 validator when the public
testnet opens.

!!! warning "The public testnet is not open"

    There is no public l1v2 network to join today. The only running network is
    the closed controlled devnet `postfiat-wan-devnet-2`
    ([current state](../status/chain-state-current.md)). This runbook
    documents, journey by journey, what a public operator will do, using only
    commands that exist in this repository now. Where a step is not yet
    possible for an outsider, a boxed gap note names the blocking item from
    the [release-gate inventory](../status/release-gate-inventory.md) or the
    milestone instead of inventing a fictional step. Track overall progress on
    the generated [testnet-path status page](../status/testnet-path.md).

## 1. Prerequisites

**Operating system.** A dedicated x86-64 Linux server with systemd. The
generated validator and RPC service units are systemd units with strict
sandboxing (`ProtectSystem=strict`, `NoNewPrivileges=true`, `UMask=0077`),
running as a dedicated `postfiat` user. No committed hardware sizing exists
yet for public operators; the controlled devnet runs six validators on
commodity cloud hosts, and the genesis specification requires that
verification work fit a reviewed commodity-hardware budget
([genesis spec §7](../architecture/l1v2-testnet-genesis-and-launch-spec.md)).

**Toolchain.** To build from source you need the Rust toolchain (`cargo`,
`rustfmt`, `clippy`) and `tmux` for the local devnet scripts. Python 3 is
needed for the repository's operator tooling; Docker and Docker Compose are
needed only for the scoring sidecar (journey 5).

**Network.** Plaintext transport and RPC listeners must bind only to loopback
or a private overlay address; the node rejects public and wildcard binds.
Public read RPC is served through an operator-managed authenticated TLS edge
that forwards to the loopback listener
([public RPC operator policy](public-rpc-operator-policy.md)).

**Get and build the source.**

```bash
git clone https://github.com/postfiatorg/postfiatl1v2
cd postfiatl1v2
scripts/check                                  # fmt + workspace compile + script syntax
cargo build --release -p postfiat-node
sha256sum target/release/postfiat-node         # record your own binary hash
```

Compare your recorded hash against the published release checksum once one
exists (see the gap note below). For the controlled devnet, the deployed
binary hash is recorded in [current state](../status/chain-state-current.md);
your locally built hash will differ unless you build the exact deployed
source revision with the same toolchain.

!!! warning "Gap: no public release artifact to download or verify"

    [Release-gate inventory](../status/release-gate-inventory.md) rows 6
    (release verification battery) and 7 (release artifacts: CycloneDX SBOM,
    signed deployment manifest, checksums, second-builder reproduction) are
    OPEN — no public-testnet release candidate has run the battery and no
    signed public download exists. Until row 7 closes, the only honest path
    is building from source as above; there is no signature or checksum file
    a stranger can verify a download against.
    [Release process](../release-process.md) steps 2–3 define what those
    artifacts will be.

## 2. Key generation and custody

l1v2 validator identity is an ML-DSA-65 (FIPS 204) key pair. The current,
real custody posture is a **plaintext software key file**:
`validator_keys.json` inside the validator data directory, protected only by
host filesystem permissions. The long-running services make this explicit —
they refuse to start without the `--unsafe-devnet-file-signer`
acknowledgement flag.

!!! danger "Open custody gap — do not place real value behind these keys"

    [Release-gate inventory](../status/release-gate-inventory.md) row 3
    (validator key custody) is OPEN: production custody — HSM/remote signer,
    or an encrypted keystore with rotation, separation, and audit logging —
    is **not implemented**. This is a recorded top gap for every public
    operator story, and
    [`SECURITY.md`](https://github.com/postfiatorg/postfiatl1v2/blob/main/SECURITY.md)
    lists it under Current Security Limitations. Nothing in this runbook
    changes that posture; treat every l1v2 validator key as a hot key on a
    testnet with no real value.

**Generate keys.** From the node binary:

```bash
target/release/postfiat-node validator-keys --data-dir /var/lib/postfiat/validator-0 --validators 1
```

This writes `validator_keys.json` under the data directory **and prints the
key records — including `private_key_hex` — to stdout**. Run it only in a
private shell on the validator host; never pipe the output into logs, tickets,
or evidence files. For a separated master/hot key pair with a signed
onboarding record, use:

```bash
target/release/postfiat-node operator-onboarding-keygen \
  --validator-id validator-0 \
  --master-key-file /secure/master.key.json \
  --validator-key-file /secure/validator.key.json
```

**Validate key files and permissions:**

```bash
target/release/postfiat-node validate-local-keys --data-dir /var/lib/postfiat/validator-0
```

The validator doctor (journey 6) also checks key-file permissions without
emitting key material.

**Practical mitigations available today:**

- Run services as a dedicated non-root user; keep the data directory `0700`
  and key files `0600`. The generated systemd units enforce `UMask=0077`.
- Never publish `validator_keys.json`, wallet backups, seeds, or raw service
  logs; the [day-two runbook](operator-day-two.md) "Do Not Publish" list and
  the redaction-checked doctor scripts are the public-evidence path.
- Stage and rotate keys with `postfiat-node validator-key-stage` and the
  registry rotation flow in the
  [emergency key rotation runbook](validator-emergency-key-rotation.md);
  rotation is a normal evidence-driven registry transition, so rotate while
  the network can still ratify it.
- Keep the deployment publisher key (release signing) strictly separate from
  validator signing keys
  ([signed deployment manifest](signed-deployment-manifest.md)).

## 3. Join the network

!!! warning "Gap: there is no public genesis to join yet"

    Joining requires a published genesis envelope (payload + ratification
    certificate), a chain ID, and a peer set. None exist for a public
    testnet: milestone items **C3** (genesis-registry proposal path) and
    **C4** (ratification client) are open, Gate Zero (**Z1–Z3**) is open, and
    the launch decision **D4** is explicitly outside the milestone's
    authority. [Release-gate inventory](../status/release-gate-inventory.md)
    row 17 (launch authority) records that no launch-authorization artifact
    is implemented. The genesis construction itself is specified in the
    locked [genesis and launch specification](../architecture/l1v2-testnet-genesis-and-launch-spec.md)
    (locked at commit `3318ab23`). The controlled devnet is closed: its
    topology and keys are operator-controlled and it accepts no outside
    validators.

**What you can rehearse today** — the join mechanics all exist locally:

```bash
# single node
target/release/postfiat-node init --data-dir .postfiat/node0 \
  --chain-id postfiat-local --node-id validator-0
target/release/postfiat-node run --unsafe-devnet-json-storage --data-dir .postfiat/node0
target/release/postfiat-node status --data-dir .postfiat/node0

# or a local multi-validator devnet
scripts/devnet-up
scripts/devnet-status
scripts/devnet-down
```

Multi-host topology files are generated with `postfiat-node topology` (or
`topology-consensus-v2 --activation-height N`), and the validator/RPC service
pair runs `transport-validator-serve` and `rpc-serve` against one data
directory — see the generated units described in the
[public RPC operator policy](public-rpc-operator-policy.md).

**First sync.** A joining node catches up from an existing RPC endpoint:

```bash
target/release/postfiat-node rpc-catch-up \
  --data-dir /var/lib/postfiat/validator-0 \
  --source-host 127.0.0.1 --source-rpc-port 27650
```

Prefer the pinned form, which fails closed unless the fetched chain ends at
exactly the tip you expect:

```bash
target/release/postfiat-node rpc-catch-up-certified-delta \
  --data-dir /var/lib/postfiat/validator-0 \
  --source-host 127.0.0.1 --source-rpc-port 27650 \
  --expected-height N --expected-block-hash HASH --expected-state-root HASH
```

Historical windows can be imported and verified with
`archive-window-verify` / `archive-window-import` / `archive-window-backfill`
(see [history retention](validator-history-retention.md)).

**Confirm you are following the chain.** All validators on one chain must
report the same chain ID, genesis hash, registry root, block tip, and state
root. Check your node and compare endpoints:

```bash
target/release/postfiat-node status --data-dir /var/lib/postfiat/validator-0
scripts/postfiat-rpc-query --endpoint validator-0=127.0.0.1:27650 --method status
scripts/testnet-rpc-doctor \
  --endpoint validator-0=127.0.0.1:27650 \
  --endpoint validator-1=127.0.0.1:27651
```

The RPC doctor checks height lag and registry-root consistency across the
endpoints you give it; divergence from the published network tip means you
are not following the chain.

## 4. Validator registration and what ratification means

l1v2 has no join transaction, no stake deposit, and no operator application
form. The validator registry is protocol state, and membership changes only
through a deterministic, evidence-driven path
([whitepaper](../whitepaper.md) §6,
[genesis spec](../architecture/l1v2-testnet-genesis-and-launch-spec.md) §3–4):

1. **Genesis membership** comes from a named, frozen Dynamic UNL scoring
   round on the PFT Ledger fork. `G0 = Selected ∩ Receipted`: you must both
   be selected by the frozen round's deterministic rules and submit a valid
   identity receipt that binds your fork secp256k1 master key to a fresh
   ML-DSA-65 validator key, before the receipt deadline. Nobody can insert,
   backfill, or waive a slot.
2. **After genesis**, registry changes flow through frozen pipeline evidence
   → a versioned transition proposal → the pinned Cobalt transition checker
   under the current registry and trust graph → commit-reveal ratification
   signatures from a quorum of current validators. Ratification is
   verification, not a vote of preference: clients sign only what
   deterministic replay confirms, and equivocation fails the round.
3. **No override authority exists.** Key loss without a replacement
   signature, proposer failure, or quorum degradation cannot lower thresholds
   or edit membership; permanent quorum loss ends in a declared halt state
   and a successor testnet, not an administrative fix.

For a new operator the practical meaning is: **the path into the l1v2
registry runs through the fork community**. Operate a PFT Ledger fork
validator, be scored by Dynamic UNL rounds
([evidence-source note](../governance/dynamic-unl-l1-evidence-source-note.md)),
and verify rounds with the sidecar (journey 5). Cobalt's role and evidence
are documented in [Cobalt](../governance/cobalt.md), the
[validator registry](../governance/validator-registry.md) design, and the
[adversarial verification results](../governance/cobalt-adversarial-verification-results.md) —
whose `KEEP_ACTIVE` decision is explicitly bounded to the controlled devnet's
validator-trust role and is **not** an operator-decentralization or
public-readiness claim. The initial transition proposer is the
Foundation-operated pipeline: an availability and censorship dependency, not
a validation authority, with a recorded follow-on milestone to remove it.

!!! warning "Gap: registration tooling does not exist yet"

    The identity-receipt flow and the ratification client are specified but
    not implemented: milestone item **C4** (extend the validator sidecar into
    the l1v2 ratification client) is open, and no qualifying frozen genesis
    round is claimed to exist
    ([genesis spec §3.1](../architecture/l1v2-testnet-genesis-and-launch-spec.md)).
    There is no command a stranger can run today to register for l1v2.

## 5. The scoring sidecar

The [validator-scoring-sidecar](https://github.com/postfiatorg/validator-scoring-sidecar)
runs alongside a PFT Ledger fork validator. Each scoring round it fetches the
foundation's frozen input package, verifies every file against the on-chain
announced hash, and can optionally re-run scoring on your own inference
runtime and participate in the fork's on-chain commit-reveal. Its own
repository documentation is the source of truth for deployment:
[`docs/Usage.md`](https://github.com/postfiatorg/validator-scoring-sidecar/blob/main/docs/Usage.md),
[`docs/Configuration.md`](https://github.com/postfiatorg/validator-scoring-sidecar/blob/main/docs/Configuration.md),
[`docs/Deployment.md`](https://github.com/postfiatorg/validator-scoring-sidecar/blob/main/docs/Deployment.md),
and [`docs/Overview.md`](https://github.com/postfiatorg/validator-scoring-sidecar/blob/main/docs/Overview.md).

When a public l1v2 operator runs it:

- **Now, verify-only:** anyone — no validator, keys, or GPU required — can
  run the published Docker image to independently verify frozen scoring
  inputs. This is the zero-risk first step toward being part of the scored
  community.
- **Now, if you operate a fork validator:** opt-in participation submits your
  validator-signed commit/reveal for scoring rounds. Signing is delegated to
  the `postfiatd validator-keys` tool; the sidecar never holds your validator
  master seed, and the on-chain fee payer is a separate funded relay wallet.
- **Later:** the same sidecar is the designated base for the l1v2
  ratification client (milestone **C4**) that replays registry transitions
  and signs commit-reveal ratifications with your ML-DSA key.

!!! warning "Gap: no l1v2 mode exists in the sidecar"

    The sidecar today serves the fork's Dynamic UNL rounds only. The l1v2
    ratification-client extension is milestone item **C4**, open. Running the
    sidecar today neither registers you with nor connects you to l1v2.

## 6. Monitoring and health

**Local validator health** — redaction-safe JSON report covering binary
checksum, state verification, registry-root availability, retention
readiness, and key-file permissions
([validator doctor runbook](validator-doctor.md)):

```bash
scripts/testnet-validator-doctor \
  --data-dir /var/lib/postfiat/validator-0 \
  --validator-service postfiat-validator-0.service \
  --rpc-service postfiat-rpc-0.service
```

**RPC health and cross-endpoint consistency:**

```bash
scripts/testnet-rpc-doctor \
  --endpoint validator-0=127.0.0.1:27650 \
  --account-address "$CANARY_ADDRESS"
```

**Cron-style monitoring snapshot** with ordered warning/critical thresholds
(height lag, RPC p95, mempool depth, certificate participation, clock skew,
connection saturation) and an optional idempotent alert spool:

```bash
scripts/testnet-monitor-snapshot \
  --endpoint validator-0=127.0.0.1:27650 \
  --account-address "$CANARY_ADDRESS" \
  --alert-spool-dir /var/lib/postfiat/monitor-alerts
```

**One-off reads** via the Python client
([python RPC client runbook](python-rpc-client.md)):

```bash
scripts/postfiat-rpc-query --endpoint validator-0=127.0.0.1:27650 --method status
target/release/postfiat-node metrics --data-dir /var/lib/postfiat/validator-0
target/release/postfiat-node account-tx-index-status --data-dir /var/lib/postfiat/validator-0
```

The read-only RPC surface (`status`, `server_info`, `metrics`, `ledger`,
`account`, `receipts`, `tx`, `blocks`, `validators`, `verify_state`, and the
rest) is enumerated in the [RPC method inventory](rpc-method-inventory.md)
and governed by the [public RPC operator policy](public-rpc-operator-policy.md).

**Logs.** The generated services append to
`/var/log/postfiat/validator-N/`: `stdout.log` / `stderr.log` (validator),
`rpc-stdout.log` / `rpc-stderr.log`, plus structured event logs
`transport-validator-events.ndjson` and `rpc-events.ndjson`. Install
`systemd/postfiat-logrotate.example` as `/etc/logrotate.d/postfiat` and
validate it with `scripts/test-postfiat-logrotate`
([day-two runbook](operator-day-two.md)).

**Failure signatures worth knowing** — each is documented in a postmortem:

| Signature in logs/reports | Meaning | Postmortem |
| --- | --- | --- |
| `peer certified batch round certificate failed: insufficient block votes: got 1, need 5` | The fleet is below finality quorum; fix validator participation before debugging anything downstream. | [WAN devnet fleet degradation](../postmortems/wan-devnet-fleet-degradation-2026-06-25.md) |
| `live validator registry activation previous validator registry root mismatch` (reason code `VALIDATOR_REGISTRY_HISTORY_REAPPLICATION_ROOT_MISMATCH`) | Fail-closed rejection of superseded validator-registry history reapplication during commit; the node refuses an invalid continuation. | [G6 rehearsal stop](../postmortems/devnet-storage-g6-rehearsal-stop-2026-08-30.md), [registry-continuation wedge](../postmortems/devnet-registry-continuation-wedge-2026-08-31.md) |
| `storage_database_error: Database already open. Cannot acquire lock.` | Two processes tried to open one transactional (`redb`) database writable; not a transient restart-order issue. | [live-canary rollback](../postmortems/devnet-storage-live-canary-rollback-2026-08-30.md) |

If an incident develops, follow the [incident response runbook](incident-response.md).

!!! warning "Gap: no public monitoring endpoints or managed alerting"

    [Release-gate inventory](../status/release-gate-inventory.md) row 18
    (operations readiness) is OPEN: production alerting, independent fault
    drills, and multi-region operations evidence do not exist. The alert
    spool above only writes local JSON events — delivery to a pager is an
    operator-supplied, separately monitored component, and there is no
    project-hosted status or monitoring endpoint a public operator can
    subscribe to.

## 7. Upgrade and rollback

Releases are defined by the [release process](../release-process.md). The
verification steps an operator can perform on a published release:

1. **Verify the signed tag and checksums** (release process step 5): a
   release is a signed annotated git tag plus signed checksums. Check the tag
   signature, then `sha256sum` your downloaded or built `postfiat-node`
   against the released checksum.
2. **Reproduce the binary** (step 3): build the exact tagged source on a
   clean builder and compare your node hash — the process requires a
   second-builder reproduction before promotion, and any operator can be that
   second builder.
3. **Verify the signed deployment manifest**: generated service units verify
   the signed manifest and the actual runtime binary, topology, and circuit
   metadata in `ExecStartPre` before every start, against the publisher's
   public key installed at `/etc/postfiat/deployment.public.json`
   ([signed deployment manifest](signed-deployment-manifest.md)).
4. **Upgrade one node at a time** with health checks between nodes. The
   supported fleet entrypoint is `scripts/postfiat-safe-rollout`
   ([safe validator rollout](safe-validator-rollout.md)): verified preflight,
   mandatory signed backup, canary-first strict ordering, per-file SHA-256
   checks, and no destructive operations. A release that changes
   replicated-state encoding additionally requires the versioned activation
   procedure in [Replicated State V2 Activation](replicated-state-v2-activation.md).
5. **Retain the rollback pair** (step 6): keep the staged prior binary and a
   schema-compatible snapshot until the post-release observation window
   closes. The [live-canary rollback postmortem](../postmortems/devnet-storage-live-canary-rollback-2026-08-30.md)
   is the cautionary case: the new binary had upgraded on-disk checkpoint
   heads, so binary rollback alone was insufficient — exact data-plus-binary
   restore was required. Take the backup before the upgrade, not after.
6. **Stop conditions** (step 4): stop and roll back on state-root divergence,
   rejected expected receipts, committee-roster mismatch, or any failed
   conservation or security invariant.

!!! warning "Gap: no public-testnet release has run this process"

    [Release-gate inventory](../status/release-gate-inventory.md) rows 6–8
    and 10 are per-release gates with no committed receipt binding them to a
    public-testnet candidate, and row 9 (protected-branch enforcement) is
    UNKNOWN. Steps 1–3 above are therefore not yet performable by an
    outsider: there is no signed public release tag, checksum set, SBOM, or
    reproduction record to verify. The commands and procedures exist; the
    artifacts do not.
