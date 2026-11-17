# NAVCoin checklist first pass, wallet round trip and one code line

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-11-17 UTC

## BLUF

Today I answered the handoff and the message of 17 November. Every one of the 40 items of the NAVCoin review checklist has a first-pass verdict (18 pass, 16 with a finding, 6 unclear), written up with evidence in a private report. I went through the public wallet by hand with real money in both directions. All B24 branches are on one private line together with main, and that line replays the chain from genesis to height 1409 on the live state root. The Solana deposit proposal has a written read. Nothing was sent to a live network except my own wallet transactions through the public site; nothing was deployed or restarted.

Primary references: [Current State](../status/chain-state-current.md), [the handoff of 13 November](2026-11-13___dravlic__b23_line_joined_with_main_and_deposit_route_checked.md).

## Current state

**The checklist** (private branch `docs/navcoin-second-set-of-eyes-20261117`, folder `docs/review/navcoin-second-set-of-eyes-20261117/`, a pull request against `docs/navcoin-handoff-solana-spec-20261117`)

- No path was found by which an outsider can take funds.
- Four points about who has to be trusted are listed there under "Before outside money".
- Each of the six unclear items says what would settle it.

**The wallet, by hand on the public Task Node site**

- [x] 2 USDC deposited and credited as PFUSDC in about 21 minutes.
- [x] 0.5 withdrawn and paid on Ethereum in about 7 minutes.
- [x] One B24 created at the price per coin plus 0.50 % and cashed in at minus 0.05 %; the rest withdrawn.
- In 2.000000 USDC, out 1.994500 USDC; the difference is exactly the two fees. Every step was also read from Ethereum and from the validators and matched the screens.
- **One defect** in the public Task Node code: `BridgeController.deposit()` (`src/features/wallet/l1/bridge/controller.js`) saves a pending record before the browser wallet is asked and clears it only when that call throws. A deposit that is started and never broadcast therefore locks the deposit screen after a reload, with no way to discard it. The same pattern is in the create flow (`primary/CreateReview.jsx`) and the withdrawal screen (`bridge/Withdraw.jsx`). A fix follows as a pull request.

**One code line** (private branch `integration/main-with-b24-20261117`, `4c78f7eac`)

- The joined line of 13 November plus eight merges in a fixed order: the live line, the on-demand reading, the withdrawal service, launch-ready, two tool branches, the handoff documents, and main up to `78fc09ad8`.
- One conflict, in `crates/node/src/transport_runtime.rs`: launch-ready's `fc7210c9d` and main's redelivery fix (`569f5f73b`) repair the same fault, a certified block whose acknowledgement is lost and whose redelivery the running build cannot answer. Both are kept in one rule.
- **Replay:** `verify-blocks` from genesis to 1409 on a read-only copy of validator-3's data ends on the state root all six validators report; `verify-finalized-checkpoint` verifies.
- **Open:** the proof input inventory fails on one pinned source, because the new reading program is not classified there yet.

**The live network** (read only; newer than [Current State](../status/chain-state-current.md), which still records 1271)

- **Last observed fleet state:** six validators at height 1419 with one state root.
- **Deployed binary and lineage:** build `abc228ed` from the private B23 line, unchanged.
- **Repository HEAD:** `main` at this handoff's commit. **Merged but undeployed:** main's changes since the B23 line split, including `569f5f73b` and `d9260e937`.
- **Live probe this session:** reads only, plus my own wallet transactions through the public site.
- **`account_tx` fails on all six for every account** ("receipt log contains duplicate tx_id"). Two governance amendment ids are linked from more than one governance block, and the running build lacks main's rule for that case (`d9260e937`), so it cannot read the account history index. Only reads are affected; the public wallet's Activity shows at most the last 64 blocks. The joined line has the rule and maintains the index.

**Validator-5's missing vote on Ethereum-side acknowledgements:** the cause and the ordered steps are in the private report (items D4 and D5). In short, the repair needs a release that carries main and one route update signed by the coin's issuer; nothing changes on Ethereum.

**The Solana deposit proposal** (`docs/specs/pfusdc-solana-alpenglow-ingress-spec.md` on the private handoff branch; the read is `solana-route-read.md` in the private report)

- The design is sound in shape.
- A realistic time is about 6 to 9 minutes until measured, because the proof and the ledger rounds remain.
- The work is larger than estimated: deposit evidence, route profiles and series ids accept only Ethereum-style data today.
- The per-epoch validator list needs its own governed, revocable action.
- A spendable balance within about 30 seconds is possible on the Ethereum route as it is, by an advance from an account that already holds PFUSDC and is repaid when the proof lands; that needs no consensus change.

**Devnet:** not touched. One read-only copy of validator-3's data was taken with a bandwidth limit and deleted.

**Task Node:** no action today (private repository work and a document drop).

## Next decision or action

Mine next:

- [ ] The wallet fix for the locked deposit screen, as a pull request to the Task Node repository.
- [ ] One page on the advance: who pays, the limits, what the user sees.
- [ ] The unclear checklist items that need nothing from the owner.
- [ ] Preparing a release from the joined line: a review of the reading program for the proof input inventory, an end-to-end redelivery test on a bounded store, the six-copy rehearsal.

Only the owner can do these (date first asked):

- a go for preparing a release from the joined line, with the rehearsal on the hosts (2026-11-17);
- after that release, one route update signed by the coin's issuer (2026-11-17);
- the evidence folder of the main machine on a private branch: stage timings, launch records, the cloud bill (2026-11-17);
- the four points under "Before outside money" in the private report (2026-11-17);
- the total spending limit of the reading job, which does not reset monthly (2026-11-17);
- a look at the deposit relay script on the main machine for a fixed expiry height (the copy on the branch passes `--expires-at-height 2000`) (2026-11-17);
- the audit firm and budget for the token contracts (2026-10-26);
- the wrapped-token path A or B and the successor's design choices (2026-11-04, 2026-11-11);
- the reserve-proof successor registration (2026-09-24);
- the corrections on website pull request 29, which still describes B23 (2026-11-13).

## References

- **Private repository:** the handoff of 17 November in `docs/handoffs/` on `docs/navcoin-handoff-solana-spec-20261117`; the report branch `docs/navcoin-second-set-of-eyes-20261117`; the joined line `integration/main-with-b24-20261117` (`4c78f7eac`) and its base `integration/main-with-b23-20261113`.
- **Main:** `569f5f73b` (redelivery acknowledgement), `d9260e937` (account history index), `78fc09ad8` (tip joined).
- **Public Task Node repository:** `src/features/wallet/l1/bridge/controller.js`, `src/features/wallet/l1/bridge/index.jsx`.
- **Status:** [Current State](../status/chain-state-current.md).
- **Previous handoff:** [2026-11-13: B23 line joined with main and deposit route checked](2026-11-13___dravlic__b23_line_joined_with_main_and_deposit_route_checked.md).
