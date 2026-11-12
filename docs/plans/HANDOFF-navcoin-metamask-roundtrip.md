# HANDOFF — Brokerage-Backed NAVCoin, USDC → pfUSDC → B23 → MetaMask

Written 2026-11-12 for the agent taking this over. Deadline is **Friday 13 Nov 2026**, investor-facing.
Read this top to bottom before touching the chain. Section 1 is a stop sign.

Repo: `/home/postfiat/repos/postfiatl1v2-navcoin-proof-b2.2` (worktree, not the main checkout).
Chain: devnet-2, six validators. We are the only users of it.

---

## 1. STOP — do not pin the beacon finality anchor

An independent review (astra, full text at `/tmp/astra-review.md`) found a **critical, unfixed** hole in
the work I did tonight. The one-line version:

> The pinned anchor authenticates the beacon *header*. It does not authenticate the *sync committee*.

What I built (`PfUsdcBeaconFinalityStateV1`, `crates/types/src/pfusdc_tier4_types.rs:251-333`) stores
`finalized_beacon_root` and `finalized_slot`, and `verify_and_advance` checks that an incoming proof's
`prior_finalized_beacon_root` / `prior_finalized_slot` match the pinned values and that the new slot
strictly advances. That closes replay-from-nowhere. It does **not** close R2-1, because the guest at
`programs/pfusdc-eth-mainnet-ingress/src/lib.rs:313-316` clones the whole witness store, and clearing
`next_sync_committee` does not authenticate `current_sync_committee`.

The attack, as stated in the review:

1. Keep the honest pinned header, so `prior_finalized_beacon_root` / `prior_finalized_slot` match exactly.
2. Replace `current_sync_committee` with 512 attacker BLS keys.
3. Sign a later header with those keys; forge tries under an invented execution state root.
4. Submit through `crates/execution/src/nav_vault_asset_execution.rs:7502-7564`.

This is not a proof-system forgery. The circuit's starting committee is attacker-selected, so the proof
is honest about a dishonest premise.

**The thing that is currently protecting the vault is that no anchor is pinned.** With no anchor the new
arm fails closed (`pfusdc_beacon_finality_state_missing`, `nav_vault_asset_execution.rs:1430-1443` and
`:1498-1506`). Pinning the anchor would *activate* the attack path while making the system look like
R2-1 was closed. I was one governance round away from doing exactly that. Do not finish that ceremony
until the committee is authenticated.

**The fix**: the pinned anchor must commit to the starting sync committee (or a complete starting-store
commitment), and the guest must take its committee from that commitment rather than from witness input.
Same shape as the P0-01 fix on the Ethereum side — ancestry must come from a pinned value, never from
the prover.

Treat "R2-1 closed" in `docs/plans/MILESTONE-navcoin-metamask-roundtrip.md` and in commit `830e75b3b`'s
message as **wrong**. It is partially closed: monotonic advancement yes, committee authentication no.

---

## 2. What actually works right now

Proven on devnet-2, end to end, buy *and* sell:

- Asset **B23** `ca7845134c8e9581…`, no `max_supply`.
- NAV epoch 1 at `$1.000000`.
- Facility `pftl-pfdep-schwab-primary-pfusdc-v1` live.
- Last verified state: height 1209, all six validators converged on root `e101f848597ed4`,
  user holds 57.999657 pfUSDC and 2013.0 B23, reserve 13.0 pfUSDC, `invariant_holds: true`.

Re-verify with `.scratch-deposit/facility.py` (status path) before trusting those numbers; they are from
the night of 2026-11-11.

The missing piece for the founder's actual ask is the **Ethereum leg**: minting the NAVCoin out to
MetaMask requires `primary_subscribe_v2` (which creates an export entitlement and takes
`ethereum_recipient`) plus a working receipt verifier on Ethereum. See §4.

### Five releases deployed tonight

All six validators, canary-first, zero divergence:

| Commit | What |
|---|---|
| `5806f08ad` | `PFTLReceiptFinalityVerifierV2.sol` — committee ancestry, checkpoint ordering, receipt replay. 14/14 tests pass. |
| `830e75b3b` | `PfUsdcBeaconFinalityStateV1` + fail-closed ingress arm. **See §1 — incomplete.** |
| `58d19f725` | Commit `beacon_finality_states` to the replicated state root (I had left it out; two validators could have held different anchors and agreed on the same root). |
| `5c74be411` | `PFTL_UNISWAP_MAX_PRIMARY_SPREAD_BPS = 500` — replaces the hard-coded `10050/9995` equality so a 20 bps route can mint. |
| `f650b32eb` → `5d13c775e` | Governance can pin an anchor; anchor bound into the signed amendment; bootstrap validated against its route profile. |

---

## 3. The governance ceremony (works, but the batch is not landing)

Sequence:

```
vault-bridge-route-profile-governance
  → governance-authorization-sign   (per validator)
  → governance-amendment-assemble
  → vault-bridge-route-profile-governance-assemble
  → transport-peer-certified-batch-round
```

Three non-obvious requirements, each learned from a rejection:

1. **The proposal slot must equal the submission height.** Build the proposal for the height you will
   actually submit at, not the height you read.
2. **The round must run on the deterministic proposer for that height.** Running it anywhere else
   returns `not deterministic proposer`.
3. **The anchor must carry the new epoch's profile hash and route binding.** Derive them and verify
   against the chain. Epoch 10 values were `b2ca6cda…` / `35ec9274…`; epoch 11 is
   `84c2aeb51bbf021d…` / `5b79e3defb54c65e…`.

**Open problem:** the certified batch round returns `round_ok: true` but the activation does not appear
in the block. Not diagnosed. Start by checking whether the batch is being accepted into the proposer's
outbox and whether the proposer at that height actually holds it.

Five rejections preceded this, all real omissions: unsigned binary (the `ExecStartPre`
`deployment-manifest-verify` gate), missing authorizations, assembled batch lacking the anchor, signed
authorization binding mismatch, lowercase 48-byte asset/manifest hash binding the consensus validator
did not know for `:beacon-bootstrap-v2:`, and `beacon finality bootstrap does not match route profile`
(my own new check firing correctly).

---

## 4. Remaining astra findings

**Finding 3 (HIGH)** — bootstrap identity vs activated route. Partially addressed in `5d13c775e`
(profile/epoch/chain/vault/token/binding checks). Still open: the lookup in
`nav_vault_asset_execution.rs` matches on `route_profile_hash` and **ignores the epoch**, and the
state-commitment sorts anchors, which hides order-dependence.

**Finding 4 (HIGH)** — P0-01 is not closed in practice. `PFTLReceiptFinalityVerifierV2.sol` exists and
passes its tests, but (a) the SP1 guest still emits version 1 with the old layout, so there is no vkey
for V2, and (b) the consumer `PFTLUniswapPrimaryMarketV2` calls `isReceiptAccepted(...)`, which V2 does
not implement. The contract is correct and unreachable. Both ends must be built before the Ethereum leg
can run.

**Finding 5 (MEDIUM)** — anchor advancement is a liveness bottleneck: a single serialised anchor means
ingress stalls behind whichever party advances it.

**Finding 2** (bootstrap not bound by governance) was raised against `f650b32eb` and is **fixed** in
`4bd388d44` / `f4a0a757b` / `5d13c775e`. Verified: the amendment kind
`vault_bridge_beacon_route_amendment_kind_v2` is present in both
`crates/types/src/shielded_bridge_governance.rs` and `crates/node/src/governance.rs`.

---

## 5. Corrections to things I said earlier

- **"The vault is paused" was wrong.** I read `0x8583409d…` out of a status doc. The live route uses
  `0xe7a432a2…`, `paused() == false`, 76.062020 USDC. It has been open and exposed the entire time.
- **validator-5 RPC was left on a pre-fix binary** by my own `/usr/local/bin/rpc5-start.sh`; its systemd
  unit was `failed` because the stray process held port 27655. Detect this via `build_git_revision`
  (it read `ae70d66d`), not by trusting the unit state.
- **My rpc5 watchdog made things worse** — restarting v5 every ~5 min at 650MB against a 2GB threshold,
  killing in-flight transport and manufacturing the stuck outbox deliveries I was then "fixing". Disabled.
- **My outbox watchdog silently did nothing** — `xargs -a file --` with no command defaults to `echo`,
  so it printed the launch command while reporting that it was restarting. Disabled.
- Three separate times I reported a false state because of bad monitoring: a buffered log (no `python -u`),
  a `pgrep` that matched my own watcher, and tracking a `timeout` wrapper PID instead of the Python child.
  Verify process state by the thing itself, not a proxy.

---

## 6. Standing exposure (the founder has accepted this)

- ~$76 in the live vault.
- ~$140 stale-NAV A666 residual.
- Fake pfUSDC exposure is **$0, specifically because no anchor is pinned** (§1).

Founder, verbatim: *"am fine dealing w vault drain"*, and *"we are the only users of this chain. do not
ask for permission again."* Do not stop the work to ask about these amounts. **Do** stop before pinning
the anchor, because that changes the exposure from $76 to the whole vault.

---

## 7. Files and state

| Path | What |
|---|---|
| `.scratch-deposit/facility.py` | Drives the whole facility: bootstrap → route-init → activate → issue (v2) → issue-native (v1) → redeem → release. `fund-pool` is SKIPPED — `pftl_uniswap_redemption_fund` is retired; primary redemption releases subscription-funded NAV reserve principal, i.e. the reserve *is* accumulated buyer money. Has a `TRANSIENT` retry list and evidence-based state recovery. |
| `.scratch-deposit/facility-state.json` | Current facility state. |
| `.scratch-deposit/status_page.py` | Renders `navcoin-status.html`. |
| `.scratch-deposit/verify_binding.py` | Derives and checks route-profile/binding hashes for the ceremony. |
| `docs/plans/MILESTONE-navcoin-metamask-roundtrip.md` | Milestone journal (AGENTS.md mandate). Its "R2-1 closed" claim is wrong — see §1. |
| `execution/SPEC-navcoin-metamask-roundtrip.md` (emergency-plan repo) | **Locked at TIH 86.00.** The requirements spec. Do not edit without re-running TIH. |
| `execution/DESIGN-brokerage-backed-navcoin.md` (emergency-plan repo) | Design doc, TIH ~79.5, 24 `[ ]` deliverables in phases A/B/C with gates C-GATE-1/2/3. |
| `/tmp/astra-review.md` | The review. 26.8KB. Read it, do not take my summary as a substitute. |

**Security cleanup still outstanding:** `~/.postfiat/trusted-deployment-publisher.private.json` is the
deployment publisher key (`pfc531e00ffefecb8fabe40c6f47c69148df31e49c`, ML-DSA-65), transferred here
over Tailscale to get a signed manifest. Delete it or return it once the ceremony is done.

Task Node `task_2e3d8ac70136562a3efaf961c2bbc567` still needs evidence (raw artifacts, not a summary).

---

## 8. Recommended order of work

1. Read `/tmp/astra-review.md` Finding 1 in full.
2. Authenticate the sync committee in the anchor + guest. Nothing else matters until this is done,
   because everything else leads to pinning the anchor.
3. Build the SP1 guest for V2 and add `isReceiptAccepted` to `PFTLReceiptFinalityVerifierV2.sol`
   (Finding 4) — the Ethereum leg cannot run without both.
4. Fix the epoch-ignoring lookup and the anchor ordering in the state commitment (Finding 3).
5. Diagnose why the certified batch round reports `round_ok: true` without landing (§3).
6. Then, and only then, pin the anchor and run the full round trip.

Do not report any step green until the user-facing outcome is verified end to end. A 6/6 commit is not
transaction success — a REJECTED receipt also commits 6/6. Check the accepted flag, the reject reason,
and the balance deltas.
