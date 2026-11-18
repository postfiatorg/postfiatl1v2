# Instant USDC in and out built, explorer checked and speed fix reviewed

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-11-18 UTC

## BLUF

Today I answered the message of 17 November evening. For the objective "much faster USDC bridging", both directions are built on private branches as an advance: an operator account pays the user at once from its own float, and the normal proof repays it later. The ledger and the vault already allow this, with no rule change. The expected time for the user is under a minute in and about 20 to 30 seconds out, against about 20 and 7 minutes measured today. Nothing is switched on: the code went through independent correctness reviews in rounds, and the pilot needs funded floats. The explorer now shows each bridge trip's Ethereum side and total time (a private branch on top of the explorer branch). I checked it against the six validators: 6,840 of 6,880 checks match and every block matches. Its display mistakes are fixed (a private indexer branch and website pull request 40). I reviewed the finality change running as release 5 and found it correct, and the joined line with it replays the chain from genesis to 1437 on the live state root. The handoff of 18 November mentioned in the message (commit `335039030` on `handoff/domagoj-usdc-latency-20261118`) is on a local branch of the main machine and was not reachable.

Primary references: [Current State](../status/chain-state-current.md), [the handoff of 17 November](2026-11-17___dravlic__navcoin_checklist_first_pass_wallet_round_trip_and_one_code_line.md).

## Current state

**Much faster USDC** (private; the design page `docs/specs/faster-usdc-in-and-out-20261118.md` on `docs/faster-usdc-20261118`)

- Measured: the deposits of 17 November took 20 min 39 s and 20 min 8 s, about 15 minutes of each being Ethereum finality. The withdrawals took 7 min 6 s and 6 min 15 s, almost all of it proving. The deposit relay's rounds stay 15 to 31 s apart after release 5.
- **Money in:** `instant/deposit-advance-20261118` (`2d5e98294`) and its follow-up `instant/deposit-advance-fix3-20261118` (`6cad89678`). The deposit is issuer-named and its nonce carries the user's account. The user is paid from the float after three Ethereum confirmations, and the claim is routed to the float. There is one service per float and a bridge job per advance. I tested it against a real signed payment whose id matches the node's own computation.
- **Money out:** `instant/withdraw-advance-20261118` (`d42ced858`). The burn names the float's Ethereum address. The user is paid with one USDC transfer after the burn is final on all six validators, and the burn is then handed to the withdraw service for repayment. Resolution is operator-only.
- Both are off by default. Each branch's README lists what is open before a pilot.
- A final independent re-read approved money out, and approved money in with three small changes to how a refused payment is proven. Those are being made on the same branch and are listed in its README.

**The explorer** (private: `ops/explorer-trip-times-20261118` (`b5b287473`) and `ops/explorer-fixes-20261118` (`ca4e2e42e`) on top of `ops/explorer-20261117`)

- I checked the trip times on four real trips (21, 7, 6 and 20 minutes).
- The check against the six validators found display mistakes only:
    - prices of A666 and BMNRC were 100× too high, from 6 instead of 8 decimals;
    - the coin table's supply was the supply at the last NAV check;
    - PFETH amounts were 1000× too high on the transaction page;
    - finished creations were shown as in progress;
    - the deposit list was cut at 60;
    - some labels claimed more than was checked, or were hidden.
- All are fixed, and the 30 explorer tests pass.
- Website pull request 40 (the page side) works with today's data service unchanged.
- Task Node pull request 410: reviewed and a comment left; the link works with a full transaction id.

**The finality change running as release 5**

- Reviewed and found correct: only identity fields of the reused status reach any decision, and the store lease is held shorter than before.
- Release preparation on `release/joined-r5-prep-20261118` (`5d3c8ffea`, `eb2ee7074`): the proof input inventory classifies the NAV reading proof (117 sources), and its check passes again. Two refusals of the vault deposit now have tests.
- **Open:** three Nitro attestation source files that the reading proof uses are not pinned yet, and the guest's program key needs a rebuild to check.

**The wallet** (public Task Node repository)

- Pull request 412: a deposit that MetaMask never sent no longer locks the screen, and recovery controls appear only when a deposit is really stuck.
- Pull request 413: the same for an unfinished Create at NAV attempt and a stalled withdrawal. A stalled withdrawal can be re-sent to the service with the same burn, never with a second burn.

**The daily price reading**

- The reading due at 17:55 UTC on 17 November did not reach the chain. B24 is still at reading 6 from block 1381; it goes stale at block 1509 under the 128-block gap.
- Private pull request 30 against `b23/nav-on-demand-20261116`: every failure leaves a record, an optional hook tells a person, and a run counts only when its reading is finalized on the chain within 10 minutes.

**Status pages:** `22b34ccc2` and `29e5f6d8f` record the 18 November read in [Current State](../status/chain-state-current.md): all six validators at 1437, one state root, release 5.

**Devnet:** not touched. Reads only, plus one read-only copy of validator-3's data for the replay, which was deleted.

**Task Node:** no action today.

## Next decision or action

Mine next:

- [ ] The open pilot items of both advance branches, and the wallet screens for them (quote, tagged deposit, burn to the float, status).
- [ ] A pilot with small amounts once the floats exist.
- [ ] Merging the reading alert and the wallet fixes after the owner's look.
- [ ] The release from the joined line: pin the three Nitro files, rebuild the reading guest to check its key, and the rehearsal on copies of the six.
- [ ] The "Storage" row of [Network Status](../status/network-status.md).

Only the owner can do these (date first asked):

- pushing `handoff/domagoj-usdc-latency-20261118` (2026-11-18);
- the two floats for instant USDC, their limits and who holds their keys (2026-11-18);
- switching the explorer's data service to the new indexer on its hosting, and merging website pull request 40 (2026-11-18);
- what broke in the 17:55 reading, and installing the reading alert on the main machine (2026-11-18);
- a go for a release from the joined line, with the rehearsal on the hosts (2026-11-17);
- after it, the issuer-signed route update (2026-11-17);
- the evidence folder on a private branch (2026-11-17);
- the points under "Before outside money" in the private report of 17 November (2026-11-17);
- the reading job's total spending limit (2026-11-17);
- the fixed expiry height 2000 in the deposit relay script on the main machine (2026-11-17);
- the audit firm and budget for the token contracts (2026-10-26).

## References

- **Private repository:** the handoff of 17 November and its reply branch `docs/navcoin-handoff-solana-spec-20261117`; the branches named above.
- **Status:** [Current State](../status/chain-state-current.md).
- **Previous handoff:** [2026-11-17: NAVCoin checklist first pass, wallet round trip and one code line](2026-11-17___dravlic__navcoin_checklist_first_pass_wallet_round_trip_and_one_code_line.md).
