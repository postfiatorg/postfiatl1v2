# pNOK private FIX demo acceptance status

Date: 2026-08-01

Scope: controlled `pfUSDC -> pNOK` private-FX demonstration

Decision: first-demo acceptance passed; Tier 4 and production readiness are
not claimed

## Outcome

The first controlled end-to-end demo is working in the browser wallet. A user
can execute an exact `20.000000 pfUSDC -> 210 pNOK` exchange at a finalized,
zero-fee `10.500000 pNOK/pfUSDC` FIX. The two input notes and both output
ownership bindings execute through Asset-Orchard privately on PFTL. The public
surface deliberately exposes the asset pair, FIX, reservation, expiry, and
final action identifiers.

The live qualification completed:

- 10 consecutive browser-initiated pNOK acquisitions;
- 9 automated inverse private swaps to restore the exact starting inventory;
- 19 unique, finalized private jobs in the repetition campaign;
- one additional private reset and acquisition under deliberate faults;
- duplicate-submit idempotency;
- validator restart recovery during the reset;
- resident prover restart and full two-circuit cold prewarm;
- wallet proxy restart during a browser acquisition, followed by wallet
  reload, unlock, and recovery of the same durable job; and
- an independent fail-closed audit with 18 of 18 checks passing.

No manual state edit was used between the 10 runs. Every acquisition reported
both input nullifiers and both outputs exactly once, unchanged pfUSDC/pNOK
supply, and rejection of an exact replay without effect.

## What is deployed

### Source sandbox

- Official upstream baseline:
  `f1ad067e09fa3e4838be9605bd1fe450831e9244`.
- Controlled bridge implementation:
  `7e293b4288279849bfe4810b25eea8d577c53bd7` on
  `feature/pnok-bridge` in the PostFiat fork of the Norges Bank CBDC
  tokenization sandbox.
- Besu is pinned to version `26.7.0` by image digest on the isolated local
  source chain.
- The controlled WNOK vault holds `500 WNOK`; its allowlist and role checks
  show no WNOK mint or burn authority and only the required transfer-from
  permission.

### PFTL

- pNOK issued supply: `500` atoms.
- pNOK counted bridge value: `500` atoms.
- Exactly one source deposit, receipt, and allocation is counted.
- The live controlled route is explicitly non-Tier-4.
- The resident Asset-Orchard service keeps both swap and private-egress
  circuits warm.
- The browser wallet discovers the asset pair and FIX from live state; pNOK
  asset IDs are not hard-coded into the component.

Epoch 3 supplied exactly 19 bounded fills for the repetition campaign and is
now filled with zero remaining capacity. Epoch 4 supplied exactly two fills
for the restart campaign and is also filled with zero remaining capacity.

After freezing the acceptance snapshot, epoch 5 was registered with 20
bounded fills and one private inverse reset restored the demo's starting
inventory. The live browser is currently `FIX VERIFIED` and `Ready to
execute`, with 19 fills remaining and an enabled exact-size swap button. This
post-qualification reset intentionally means that rerunning the point-in-time
acceptance auditor against mutable live note inventory will not reproduce its
final-acquisition inventory check until another acquisition completes.

## Browser qualification

The repetition campaign ran from `06:58:16Z` to `07:40:47Z`, 2,551 seconds
for 10 acquisitions and 9 inverse resets. Individual acquisitions took
approximately 148 to 156 seconds, including the explicit replay-without-effect
verification. Inverse resets took approximately 114 to 138 seconds.

These measurements are evidence, not a sub-minute performance claim. The
first-demo specification does not impose a latency gate below 20 seconds.

The qualified final screen shows:

- `FIX SETTLED` and `Private swap complete`;
- exact `20.000000 pfUSDC` input and `210 pNOK` output;
- zero fee and zero price impact;
- the finalized epoch, expiry, packet, and PFTL height;
- zero remaining bounded fills;
- `private on PFTL`; and
- `controlled sandbox checkpoint`.

Navigation, refresh, lock/unlock, proxy restart, and process recovery retain
the durable job and rebuild this screen from finalized PFTL state. A filled
market is rendered as settled rather than incorrectly rendered as blocked.

## Recovery findings

The deliberate restart campaign passed. The browser acquisition completed
with `retry_count = 2` after the proxy was killed mid-job. The resident prover
cold-started both K=15 circuits in approximately 323 seconds on 32 threads.
This cold-start delay is operationally significant; the demo service must stay
resident and warm before use.

The fault harness itself initially exposed two false assumptions and was
corrected before the accepted run:

1. one-direction inventory returns an intentional HTTP 503 readiness status
   even though its structured payload is valid for a reset; and
2. authenticated mutations must be dispatched from the same-origin browser,
   not directly from a Node test process.

Both rejected attempts stopped before a mutation. The accepted campaign uses
the actual browser origin and treats directional readiness explicitly.

## Evidence

- First-demo acceptance report:
  `deployments/pnok-private-fix-20260801/acceptance/public/report.json`
- Ten-run aggregate:
  `deployments/pnok-private-fix-20260801/browser-qualification-10x/report.json`
- Fault-recovery aggregate:
  `deployments/pnok-private-fix-20260801/recovery-faults/report.json`
- Final qualified browser screenshot:
  `deployments/pnok-private-fix-20260801/recovery-faults/04-recovered-private-swap-qualified.png`
- Epoch 3 packet and status:
  `deployments/pnok-private-fix-20260801/repeat-fix-epoch-3/public/`
- Epoch 4 packet and status:
  `deployments/pnok-private-fix-20260801/repeat-fix-epoch-4/public/`
- Epoch 5 packet, demo-ready status, and browser screenshot:
  `deployments/pnok-private-fix-20260801/repeat-fix-epoch-5/public/`

The acceptance auditor also reruns the expired-FIX regression and scans public
artifacts for custody material. Its report is mode `0600` and contains no note
openings, owner addresses, secret keys, seeds, or wallet custody material.

## Explicit limitations

This result is a controlled demo, not a production or Tier-4 bridge:

- the source checkpoint is controlled rather than proven under continuous
  Besu/QBFT finality;
- the isolated runner authorizes both test participants;
- this is not an official Norges Bank FIX or endorsed Norges Bank deployment;
- live pNOK-to-WNOK release was not executed in this qualification; the release
  contract path and conservation/replay behavior passed Solidity and
  integration tests; and
- unattended public users, a 100+ swap campaign, independent security review,
  and Tier-4 ingress/egress remain separate follow-on gates.

## Reproduction commands

From the repository root:

```bash
cd wallet-web
node scripts/live-pnok-private-fix-campaign.mjs
node scripts/live-pnok-private-fix-recovery-faults.mjs
cd ..
python3 scripts/pnok-first-demo-acceptance-audit.py
```

The campaigns consume deliberately bounded FIX capacity. Register a reviewed
successor epoch before rerunning them; do not edit live reservation, note, or
fill state manually.
