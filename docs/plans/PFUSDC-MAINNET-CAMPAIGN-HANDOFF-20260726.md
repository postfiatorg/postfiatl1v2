# pfUSDC Ethereum Campaign — FULL HANDOFF

**Written:** 2026-07-26T14:30Z by Angmar (Nazgul command), on Sauron's order.
**Reason:** Troll pane (Burzum) wedged on repeated content-filter turn errors;
orc panes idle/stopped. This document is sufficient to resume with a fresh
hierarchy or a single agent. Read it completely before touching anything.

## Program history and binding product requirements — 2026-07-27

This section records the product lineage that led to the Ethereum-mainnet
pfUSDC rail and the downstream a666 requirement. It is the canonical
interpretation of the campaign. The `NO a666 work` statement in Section 1
scoped the pfUSDC campaign acceptance test; it does not mean that pfUSDC alone
is the end-user product.

### 1. Arbitrum was attempted and rejected because trustless finality took about six days

The first Tier-4 pfUSDC design used canonical USDC on Arbitrum One. PFTL would
verify the deposit output in an Arbitrum Nitro assertion `sendRoot`, then prove
that the assertion was confirmed under finalized Ethereum state. This was the
correct no-trust boundary, but a Nitro assertion required approximately 6.4
days to become usable under that boundary.

That delay made the route commercially unusable against the required
25-minute user journey. The Arbitrum attempt therefore failed as a product
route even though its proof architecture was technically coherent. Arbitrum
is deprecated as a pfUSDC ingress domain: do not register or activate a new
Arbitrum route profile. Historical Arbitrum balances remain explicitly
accounted for as legacy conservation terms, not usable capacity.

Source design:
`docs/plans/PFUSDC-TIER4-IMPLEMENTATION-PLAN-20260717.md`. Deprecation
decision:
`docs/plans/A666-MAINNET-TRUSTLESS-MINT-SPEC-20260725.md`.

### 2. Ethereum-mainnet pfUSDC was subsequently implemented and succeeded

The replacement rail starts with canonical Ethereum-mainnet USDC:

```text
Ethereum USDC approval and vault deposit
  -> finalized Ethereum evidence
  -> SP1 Groth16 ingress proof
  -> PFTL proposal/finalization
  -> spendable pfUSDC credited to the named PFTL recipient
```

This path succeeded with real mainnet USDC and the six-validator PFTL fleet:

- The 25 USDC campaign completed the entire ingress, PFTL transfer/privacy,
  burn, proof, and mainnet withdrawal round trip with exact conservation and
  replay rejection. Its first run was functionally correct but took
  2h45m48s, so it failed the latency requirement.
- The replacement 1 USDC run completed deposit inclusion through withdrawal
  inclusion in 20m12s, passing the 25-minute requirement with 4m48s of
  margin.

Observed Ethereum cost to move USDC into spendable PFTL pfUSDC was:

| Mainnet run | Approval gas | Deposit gas | Total ETH | USD at campaign-pinned $1,874.50/ETH |
|---|---:|---:|---:|---:|
| 25 USDC functional run | 55,570 | 215,669 | 0.000015740899 ETH | about $0.03 |
| 1 USDC latency run | 55,558 | 215,645 | 0.000081604399 ETH | about $0.15 |

The gas amount was approximately 271,000 in both runs; the ETH cost varied
with the mainnet gas price. These figures cover the user's ERC-20 approval and
vault deposit. They exclude off-chain proof-generation compute, contract
deployment, and the later Ethereum withdrawal. Deposit gas is substantially
amount-independent, so a larger USDC principal should be in the same gas class,
subject to the live gas price and exact transaction path.

Canonical evidence:
`docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/recovery-epoch4/closing/campaign-summary.json`,
`docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/recovery-epoch4/deposit/deposit-result.json`,
`docs/evidence/pfusdc-eth-mainnet-latency-20260727-run2/roundtrip-summary.json`,
`docs/evidence/pfusdc-eth-mainnet-latency-20260727-run2/deposit/deposit-result.json`,
and
`docs/evidence/pfusdc-eth-mainnet-latency-20260727-run2/latency-gate.json`.

### 3. pfUSDC must increase NAVCoin supply; it must not buy finite OTC inventory

The large-capacity product is primary issuance, not an OTC swap against
Alex's existing NAVCoin inventory. Bob must not have to find Alex, negotiate
with Alex, or consume a finite offer of pre-existing a666 in exchange for
pfUSDC.

The required state transition is:

```text
verified pfUSDC deposit
  -> user-signed primary subscription at finalized pre-inflow NAV
  -> pfUSDC base value debited into counted reserve; spread separately booked
  -> authorized valid a666 supply increased
  -> Bob's spendable a666 balance increased atomically
```

At NAV = $1.00 and a mint price of `NAV x 1.005`, Bob pays 100,500 USDC
to create 100,000 new a666. The 100,000-USDC base value becomes counted NAV
reserve and the 500-USDC spread is separately accounted outside NAV assets,
so reserve value and liability grow together without changing the stated
$1.00 NAV. The amount does not traverse an AMM curve. A 30,000 USD Uniswap
pool must not constrain a 100,000 USD primary subscription.

Exporting the result to Ethereum is a representation move, not a second
economic issuance: PFTL a666 is debited or made unspendable before the exact
amount of wA666 can be minted. The cross-venue conservation identity must
remain true after every transition.

Binding capacity requirement:

- support at least 2,000,000 a666 of posted primary mint capacity;
- support a 100,000 a666 export in one packet or one atomic batch with the
  same all-or-nothing guarantee;
- derive available mint capacity from proven backing and policy caps, never
  from Uniswap liquidity or issuer inventory; and
- support the inverse primary redemption, which retires NAVCoin supply and
  releases the corresponding settlement value under the posted redemption
  band.

The settlement principal used by that redemption is the base pfUSDC value
contributed through primary subscriptions and held in NAV reserve custody.
The 2,000,000-a666 redemption-capacity parameter is a policy ceiling, not a
requirement to prefund a second 2,000,000-pfUSDC redemption inventory.

The current controlled configuration does not meet this requirement. With six
decimal places, its 10,000,000-atom route cap is 10 a666, its
1,000,000-atom packet cap is 1 a666, and the current native asset definition's
1,000,000,000,000-atom maximum is 1,000,000 a666. Those are test-era limits.
Production must use a fresh a666 asset version with `max_supply` absent
(`None`), plus a production route whose active exposure and packet caps
support the posted facility. The 2,000,000-a666 number is a governed,
proof-backed issuance/redemption capacity, not a permanent lifetime supply
ceiling. The binding design is
`A666-END-TO-END-MAINNET-PRIMARY-ISSUANCE-SPEC-20260727.md`.

### 4. a651 is legacy; a new mainnet wA666/USDC pool is required and does not yet exist

The Ethereum-mainnet a651 token and a651/USDC Uniswap v4 pool are real legacy
deployments. They were launched as a standalone Ethereum venue seeded near
NAV. They were never a trustless bridge from the PFTL a651 supply ledger:
the legacy `NavBridgeController` used owner-authorized burn-here/mint-there
operations and did not verify PFTL finality. A later read-only StateView check
reported zero pool-specific liquidity.

The a651 token, controller, and pool must remain historical and
inspection-only. They must not be relabeled, repointed, or used as a fallback
for the a666 product.

The required Ethereum venue is a new bridge-aware ERC-20 representation,
`wA666`, paired with canonical mainnet USDC in a new Uniswap pool. Its seed
wA666 must come from a valid PFTL primary subscription followed by a verified
export; it must not come from an unexplained manual Ethereum mint.

**Current mainnet status: NOT DEPLOYED.** There is no persistent public
Ethereum-mainnet wA666 token/controller and no live mainnet wA666/USDC or
a666/USDC Uniswap pool. Controlled local/fork rehearsals and a controlled
Sepolia stack exist, but they are not mainnet deployments and are not a
trustless public route.

The new pool is a price anchor and secondary venue. Large acquisition must use
the primary issuance path in Section 3 and deliver wA666 directly to Bob's
mainnet address without forcing the 100,000 USD order through the pool.

Canonical legacy record: `docs/navcoins/uniswap-pool.md`. Replacement
architecture and current gate record:
`docs/plans/pftl-uniswap-bridge-redeployment-spec.md` and
`docs/plans/A666-END-TO-END-MAINNET-PRIMARY-ISSUANCE-SPEC-20260727.md`.

## Private pfUSDC send acceptance closure — 2026-07-27

**Status: COMPLETE / PASS.**

A real 100,000-atom (0.1 pfUSDC) private send finalized on the six-validator
PFTL fleet:

- H334 shielded 100,000 transparent pfUSDC into an Asset-Orchard note owned by
  wallet A.
- H335 shielded a 100,000-atom a651 helper note owned by wallet A.
- H336 consumed both input nullifiers and created an exact 100,000-atom
  pfUSDC note recoverable and spendable only by distinct wallet B. The a651
  helper value remained an unspent, spendable private note controlled by
  wallet A.
- An exact replay of the finalized batch was rejected as already applied.
- All six validators converged at H336 on one tip and one state root with
  empty mempools.
- The public action contains no raw asset ID, amount, sender, recipient, memo,
  spending key, or recipient seed. Stable asset tags and the 1:1 pricing claim
  remain public by current circuit design.

This test uses the existing fixed two-input/two-output Asset-Orchard swap
circuit as a narrow private-send construction: a same-value a651 helper note
occupies the second input/output slot while pfUSDC moves from wallet A to
wallet B. It proves the requested live pfUSDC movement and conservation, but
does not replace the planned general one-input typed-transfer primitive.
Ingress remains a public boundary.

Cold proof construction took about 7m56s. The finalized H336 consensus round
took 34.03s total and reported 27.52s client-visible finality.

Canonical evidence:
`docs/evidence/pfusdc-private-send-20260727-pfusdc-private-send-01/gate.json`,
`private-send-verification.json`, `fleet-attestation.json`, and the hashed
public action/batch in the same directory. The resumable fail-closed harness is
`scripts/pfusdc-private-send-live`.

## Latency acceptance closure — 2026-07-27

**Campaign status: COMPLETE / PASS.**

The replacement live Ethereum-mainnet round trip passed every required gate:

- Route: Ethereum USDC -> PFTL pfUSDC -> Ethereum USDC.
- Amount: 1,000,000 atoms (1 USDC).
- Deposit inclusion: block 25,624,516 at Unix timestamp 1,785,159,767.
- Withdrawal inclusion: block 25,624,616 at Unix timestamp 1,785,160,979.
- End-to-end elapsed time: 1,212 seconds (20m12s).
- Maximum permitted time: 1,500 seconds (25m).
- Margin: 288 seconds (4m48s).
- Ethereum finality ingress proof, PFTL propose/finalize/claim/burn, egress
  proof, exact USDC deltas, replay rejection, six-validator convergence, empty
  mempools, and conservation delta-zero: `PASS`.

Canonical evidence:
`docs/evidence/pfusdc-eth-mainnet-latency-20260727-run2/roundtrip-summary.json`
and
`docs/evidence/pfusdc-eth-mainnet-latency-20260727-run2/latency-gate.json`.

## Latency acceptance correction — 2026-07-27

**Historical status before the replacement run: FUNCTIONAL PASS / LATENCY
FAIL.**

The completion update below correctly records proof validity, exact value
movement, replay rejection, fleet convergence, and conservation. It did not
measure the required end-to-end latency and therefore must not be read as full
campaign acceptance.

- Required boundary: Ethereum deposit inclusion through Ethereum withdrawal
  inclusion.
- Required maximum: 1,500 seconds (25 minutes).
- Observed deposit: block 25,619,777 at Unix timestamp 1,785,102,719.
- Observed withdrawal: block 25,620,606 at Unix timestamp 1,785,112,667.
- Observed elapsed time: 9,948 seconds (2h45m48s).
- Correct verdict: latency `FAIL`.

Canonical correction:
`docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/recovery-epoch4/closing/latency-gate-20260727.json`.
The fail-closed checker is `scripts/pfusdc-mainnet-latency-gate.py`.

The next attempt must pass the pre-deposit checkpoint-freshness gate, use the
CUDA prover path, and receive a `PASS` from the checker before the campaign or
release is described as complete.

## Completion update — 2026-07-27

**Historical functional status: COMPLETE / PASS.** This predates the latency
acceptance correction above. The recovery procedure below is retained as
historical context and must not be replayed.

- Corrected epoch-4 vault and verifier deployed at
  `0x8583409ddbac984ec195dfa06a21103d92403c1e` and
  `0xa77d5af456ef212303e31727b6ca4888cd771e2c`.
- 25 USDC entered from Ethereum mainnet, was claimed as spendable pfUSDC,
  traversed transparent and Orchard transfers, burned, proven with the pinned
  Groth16 egress verifier, and exited as 25 USDC to a different Ethereum
  recipient.
- Final withdrawal transaction:
  `0x88892905cf60a5c4367f26289c1080ba30cf7b6a8490eb33a15f1f1644d491d8`
  at Ethereum block 25,620,606. The vault delta was -25,000,000 atoms and the
  recipient delta was +25,000,000 atoms.
- Replay was rejected and the withdrawal, burn, proof nullifier, and Orchard
  nullifier were consumed exactly once.
- All six PFTL validators converged at H326 with identical tip/state root and
  empty mempools.
- Closing conservation residual equals the opening residual exactly; delta is
  zero.
- Realized campaign gas was approximately $1.19 at the pinned ETH/USD price;
  the conservative all-in accounting remained below the $250 cap.
- Temporary prover rentals were destroyed and private campaign copies were
  removed after the public evidence was preserved.

Canonical closing record:
`docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/recovery-epoch4/closing/campaign-summary.json`.
Withdrawal receipt and balance/replay checks:
`docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/recovery-epoch4/egress/withdrawal-result.json`.

## 1. Mission (Sauron's directive, unchanged)

Bridge real USDC from an Ethereum wallet into pfUSDC on the PFTL fleet
(proof-gated, trustless), then prove four legs end-to-end with measurements:

1. Ingress: USDC deposit -> SP1 ingress proof -> pfUSDC spendable at claim.
2. Transparent pfUSDC send wallet A -> B (must spend newly minted atoms).
3. Orchard shielded pfUSDC send A -> C (nullifier evidence).
4. Egress: burn -> exit leaf -> SP1 finality proof -> batch exit root ->
   Merkle claim -> USDC to a DIFFERENT recipient.

Acceptance: per-leg wall-clock to spendable, exact balance deltas, proof cost
(guest cycles / proving wall-clock / peak RAM), wallet UX shows correct
balances, conservation delta-zero (Section 6). NO navswaps, NO a666 work.
**Venue is Ethereum MAINNET with live USDC** (Sauron order ~02:55Z).
Arbitrum is deprecated forever; never register an Arbitrum route profile.
The full Ethereum deposit-inclusion to withdrawal-inclusion wall clock must be
at most 1,500 seconds.

## 2. Hard constraints from Sauron

1. **Budget: $250 realized-spend cap, total.** The stranded 100 USDC
   (Section 4) is recorded as recoverable surplus backing, not realized
   spend; the retry deposit is REDUCED to 25 USDC so worst case stays at
   the cap. Track running gas in USD in every report.
2. **Never block on Sauron.** No plan or ETA may include a Sauron-executed
   step (the faucet incident). Decisions/authorizations only. If truly
   blocked, report "blocked on X" with no delivery promise attached.
3. Get it done. Velocity over process polish, but fail-closed gates stay.

## 3. DONE and verified — do not redo

Evidence root: `docs/evidence/pfusdc-eth-campaign-20260725/`

1. **Fleet:** six validators on Ethereum-capable candidate
   `0467b6b8…7b56` (git head `f30d368`), full signed-snapshot rollout,
   per-host evidence in `lane-a/rollout-stage/`, acceptance
   `lane-a/acceptance.log` ("all WS1 Lane A gates proven").
2. **Sepolia rehearsal rail:** rev6 contracts nonce-67/68 live and
   correctly wired; route `ethereum-sepolia-usdc-v1` registered+activated
   epoch 2 at PFTL H314 (H311 bind, H312 first attempt rejected in
   execution — never registered; H313/H314 landed). Arbitrum-ingress
   negative test PASSED (deprecation enforced by chain state).
3. **Mainnet PFTL side:** H315 NAV profile+bind batch VERIFIED GREEN;
   H316 epoch-3 mainnet route registration/activation PASS
   (`lane-mainnet/pftl-execution/02-h316-register/h316-summary.json`);
   residual gate PASS.
4. **Mainnet contracts deployed:** vault `0x47d54874…940af9` (full address
   in `lane-mainnet/deploy/`), verifier per same manifest; digest-gated
   deploy, two-key audited.
5. **Mainnet deposit landed:** 100 USDC, tx
   `bdb3407204cc1fa1791ec16e727dee96eedde59b2b2c43fa87b263880a8f9764`,
   block 25,617,296, status 1, deposit ID `0x3c9067e0…57233d`. Deposit
   builder was consumer-vector validated (fixed after the Sepolia
   byte-order strand; see `lane-b/diagnostics/h315-deposit-formula-*`).
6. **Conservation instrument:** checker + signed-snapshot import method
   proven; H310 baseline residual +5,000,010 (Arbitrum legacy surplus,
   pinned constant); Sepolia strand 357,559 atoms documented
   (`lane-b/diagnostics/…/recovery-math.md`); tri-state verdicts
   (verified/violated/execution_blocked); ABI selection data-driven from
   `deployments/pfusdc-vault-interface-lineage-20260725.json`.
7. **Tooling hardened tonight:** deterministic package GENERATOR (never
   hand-edit manifests), value-level stale scans, offline `--check-only`
   deploy validation (no Web3 init), AST-derived consumer schema coverage,
   **mechanical digest-gated sender** (broadcast refuses any manifest not
   matching a complete passing from-zero audit).

## 4. THE BLOCKER — contract/guest storage incompatibility

Authoritative analysis: `lane-mainnet/e2e/02-ingress-proof/`
`contract-guest-storage-incompatibility-report.v2.md` (Snaga, verified).

- Deployed mainnet vault `depositV2` persists ONLY
  `depositSeen[depositId]=true` (mapping at slot 3); all deposit fields are
  event-only.
- The FROZEN mainnet ingress guest
  (`programs/pfusdc-eth-mainnet-ingress/src/lib.rs:27,109-175`) expects a
  FIVE-slot storage record under mapping slot 1 (obligations,
  packed depositor||amount, recipient_hash, route_binding, nonce).
- Therefore no storage proof can ever satisfy the guest for this vault:
  the record does not exist on chain. Verified three ways with
  `eth_getStorageAt`/`eth_getProof` readbacks (see the report's table).
- Additionally, the landed deposit's event carries the epoch-3 route
  binding; PFTL active-ingress admission
  (`crates/node/src/execution_actions.rs:130-163`) accepts only evidence
  bound to the ACTIVE profile/epoch. So the 100 USDC is UNCLAIMABLE under
  current consensus regardless of new guests — it is recorded as mainnet
  surplus backing (recoverable economically via the redemption flow).
- Receipt-trie alternative-evidence workstream: CLOSED as moot (stop
  records in `lane-mainnet/reviews/receipt-trie-recovery-feasibility/`;
  four environment failures documented; do not resurrect without a ruling).

## 5. RULED RECOVERY PATH (dispatched, not yet executed)

Fix-forward. No consensus changes, no migration ops, no gate relaxation.

1. **Contract fix:** modify `ERC20BridgeVaultL1.depositV2`
   (`crates/ethereum-contracts/src/ERC20BridgeVaultL1.sol:112-145`) to
   persist the exact five-slot record the frozen guest expects, mapping
   slot 1, byte-for-byte per guest decode. THE CONTRACT MOVES TO THE
   GUEST, never the reverse (guest/vkey stays frozen). Forge tests must
   assert layout via `forge inspect storage-layout` AND simulate the
   guest's decode path against a local deposit.
2. **New permanent gate:** contract<->guest STORAGE CROSS-CHECK in the
   generator/verifier (forge layout vs guest constants, mechanical,
   fail-closed). Absence of this gate caused the blocker.
3. Regenerate package (new vault runtime hash, epoch 4) with the standard
   pipeline: generator -> value scans -> both offline sims (PFTL admission
   `execution_actions.rs:958-982` + consumer C8 exact-match) -> deploy-tool
   AST schema -> digest-bound from-zero self-audit.
4. Digest-gated deploy of the corrected vault (+verifier only if its
   commitment must change — it bakes vault runtime hash, so YES a new
   verifier pair; quarantine the old pair with reason).
5. PFTL epoch-4 sequence, same proven pattern: H_next NAV profile+bind
   batch, H_next+1 route register/activate (activation height MUST equal
   registration height — protocol forbids future activation;
   `execution_actions.rs:940-944`; bind strictly precedes registration).
   Chain is ACTIVITY-DRIVEN (no empty blocks;
   `crates/node/src/mempool_proposals.rs:2393`): heights advance only via
   our own transactions; pin heights, halt on divergence, never inject
   filler transactions.
6. **25 USDC deposit** via the validated builder; recompute the consumer
   vector against the LIVE deposit event BEFORE starting the proof job.
7. Mainnet finality (~15 min) -> ingress proof -> claim -> transparent
   send -> Orchard send -> burn -> egress proof -> USDC out to a different
   recipient. Egress: keep verifier checkpoints fresh so the proven
   segment stays short (the 2026-07-18 postmortem OOM'd on a 25-block
   segment; 2.03B cycles, 109 GB RAM — do not repeat).

## 6. Conservation bookkeeping (delta-zero)

Identity `V = S + D + B - R`. Acceptance: residual at closing bracket
minus residual at opening bracket == 0 EXACTLY. Pinned constants (never
"fitted away", each with evidence): Arbitrum-One legacy surplus
+5,000,010 atoms (vault `0x850e4cee…fb58`, chain 42161, read-only,
one-time-labeled `deprecated_arbitrum_legacy`); Sepolia stranded deposit
357,559 atoms (nonce-68 vault, no sweep function exists); mainnet stranded
100 USDC (Section 4). Any change in a pinned term = outside interference =
halt. Legacy route facts: `findings/legacy-arbitrum-one-active-route.md`
(genuine SP1, expiry height 100000, no deactivation mechanism, superseded
for ingress by active-route selection).

## 7. Resources and credentials (PATHS ONLY — never print values)

- Ethereum deployer `0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0`:
  ~0.29 ETH + ~3,362 USDC on mainnet (recheck live), ~4 ETH Sepolia.
  Signing via agentd only; no raw key handling.
- Vultr API key file: `/existing/secret-safe/vultr.txt` (0600, one token).
- Snapshot publisher key:
  `/existing/secret-safe/recovery-v3-snapshot-publisher.private.json`.
- Deployment publisher key (schema
  `postfiat.deployment_publisher_private_key.v1`):
  `/existing/secret-safe/deployment-publisher.private.json`.
- pfUSDC issuer key: exists on fleet hosts; Sauron ruled NO rotation after
  the 2026-07-26 leak incident (file deleted, never committed — incident
  record in `lane-c/incidents/`). Fleet-artifact copies must EXCLUDE key
  files.
- Fleet: six Vultr validators, inventory
  `/existing/secret-safe/validator-inventory.txt`; access pattern in
  `lane-b/fleet-access-method.md`. Rollout tooling:
  `python/postfiat_ops/safe_rollout.py`.
- Canonical mainnet USDC `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48`;
  canonical Sepolia USDC `0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238`.
  Mock tokens prohibited.

## 8. Discipline that MUST survive the handoff

1. Single-agent authorization (directive amended 2026-07-26): the executing
   agent may authorize its own build only through the complete digest-bound
   from-zero audit. The sender mechanically refuses a missing, stale,
   incomplete, or failing audit.
2. Consumers are the spec: before ANY submission, simulate every real
   consumer offline — PFTL admission, C8 exact-match, deploy-tool schema,
   and the new contract<->guest storage cross-check.
3. Generator-only packaging. Hand-edited manifests caused three defect
   cycles and one near-miss broadcast. Regenerate; never patch.
4. Append-only evidence. Corrections are dated appendices; originals stay.
5. Shared tree: one owner per artifact; no destructive git commands;
   foreign-path anomalies are REPORTS, not cleanup targets.
6. Stop predicates: digest mismatch, pinned-height divergence, live
   predicate vs simulation divergence, readback mismatch, residual
   movement, evidence self-contradiction, budget breach ($130 projected /
   $250 realized). A fired predicate halts and escalates; nothing else
   stops for permission.
7. Failure-class catalog (all recurred at least once — check for them):
   mixed-revision stale values; consumer-schema gaps; derived-hash
   inheritance across revisions (re-derive, never copy); byte-order in
   deposit-ID computation; storage-layout drift between contract and
   guest.

## 9. Immediate next actions, in order

1. Execute Section 5 steps 1-3 (all local, $0): contract fix + storage
   cross-check gate + regenerated epoch-4 package + from-zero audit.
2. Digest-gated deploy (~$40 gas) + readbacks + postdeploy audit.
3. PFTL epoch-4 registration sequence (free, minutes).
4. 25 USDC deposit -> consumer-vector recheck -> proof -> claim.
5. Four legs with full measurement discipline; conservation brackets.
6. Deliver to Sauron: per-leg receipts, timing table, proof costs, gas
   total vs $250 cap, and the delta-zero verdict.

Estimated: ~2h local work, then ~1.5-2h of protocol physics (finality +
two SP1 proofs). Nothing in this path requires Sauron.
