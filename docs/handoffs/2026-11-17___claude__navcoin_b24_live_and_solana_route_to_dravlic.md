# NAVCoin B24 live on the public Task Node, and a proposed Solana deposit route

- **To:** Domagoj Ravlić (`dravlic`)
- **From:** Claude, working for the founder on the main machine (`postfiatfoundationv2`)
- **Date:** 2026-11-17 UTC

## BLUF

B24, the Schwab-backed NAVCoin, is live end to end with real money on
`postfiat-wan-devnet-2` and on Ethereum mainnet, and the wallet for it is on
`https://tasknode.postfiat.org` behind an "Experimental" label. On 2026-11-16 the founder's
7,025 USDC went through the full path: USDC to PFUSDC, PFUSDC to B24 at NAV, B24 to wB24 on
Ethereum, and a Uniswap v4 pool seeded with 5,000 wB24 and 2,000 USDC. Deposits and
withdrawals of USDC have both paid out with real funds. The path works; it is slow (about 26
minutes for a deposit) and it is operated by scripts and services on one machine with one
person's knowledge. This document is so that knowledge is no longer in one place.

Two things are asked of you:

1. **Review.** A checklist of 40 review items is in
   [`docs/review/navcoin-second-set-of-eyes-checklist-20261116.md`](../review/navcoin-second-set-of-eyes-checklist-20261116.md).
   Nothing in it has an independent verdict yet.
2. **A decision on the Solana route.** The spec is
   [`docs/specs/pfusdc-solana-alpenglow-ingress-spec.md`](../specs/pfusdc-solana-alpenglow-ingress-spec.md).
   It would cut deposit time from about 26 minutes to about 5 to 11. It needs a consensus
   change, so it needs your read before anything is built.

Nothing here was written by you or reviewed by you. Treat every claim as unverified until you
have checked it; the evidence paths are given so that you can.

## Where things are

**Code.** The chain work is on branches of
`postfiatorg/postfiatl1v2-private-archive-20260717`, not on `postfiatorg/postfiatl1v2`
`main`. None of it is merged to a main line. Each branch has a worktree on the main machine
under `/home/postfiat/repos/`.

| Branch | Head | Worktree | What it holds |
| --- | --- | --- | --- |
| `b23/tee-phase6-mainnet-20261114` | `43ddc7d6` | `postfiatl1v2-b24-tee-nav` | The network software the live validators run, the Ethereum contracts (`ops/b24-replacement/contracts`), the B24 daily reading |
| `b23/nav-on-demand-20261116` | `e9eecd10` | `postfiatl1v2-now` | On-demand NAV reading (`ops/b24-live/daily.py --now`) and the pool launch scripts (`ops/b24-live/launch_*.py`) |
| `b23/withdraw-20261116` | `4b7f76b6` | `postfiatl1v2-withdraw` | PFUSDC to USDC payout service and starter-PFT faucet service (`ops/withdraw/`) |
| `b23/launch-ready-20261116` | `b07e1ea0` | `postfiatl1v2-ready` | Launch rehearsal tooling |
| `b23/ops` | `87d38ab4` | `postfiatl1v2-b23-ops` | Bridge job client (`ops/b23/bridge_job.py`) |
| `docs/navcoin-handoff-solana-spec-20261117` | this branch | `postfiatl1v2-handoff-20261117` | This handoff, the Solana spec, the review checklist |

- Schwab reader and enclave: `postfiatorg/navstrategies`, branch `b23/tee-nav-phase1-20261113`,
  head `f6d6be7f`, worktree `navstrategies-b23-tee-nav`.
- Task Node wallet: `postfiatorg/tasknode` `main`. NAVCoin work landed as PRs #403, #407 and
  #408. Server code is in `server/l1/`, the wallet in `src/features/wallet/l1/`.
- Raw evidence and worker records: `/home/postfiat/repos/b23-coord/evidence/` and
  `/home/postfiat/repos/b23-coord/status/`. This folder is not a git repository and exists
  only on the main machine.

**Public pages.**
- Wallet: `https://tasknode.postfiat.org` (sign in, then the L1 wallet).
- Coin data without sign-in: `https://tasknode.postfiat.org/api/l1/navcoin/profiles/b24`
  (also `a666`, `bmnrc`, `b23`).
- Public write-up with diagrams: `https://postfiat.org/research/schwab-navcoin-handoff/`.
  It predates the pool launch and is out of date on supply and pool figures.

## State of the network (read 2026-11-17 01:06 UTC)

| Item | Value |
| --- | --- |
| Network | `postfiat-wan-devnet-2`, six validators, height 1400 |
| B24 asset | `52285e4d…98f7d8af`, route `pftl-b24-ethereum-wB24-usdc-v1` |
| B24 NAV | 0.999922 USD, reading epoch 6, submitted at height 1381 |
| B24 supply | 420,322.38 on the network plus 5,001 wrapped on Ethereum |
| PFUSDC family / series S | `02c46a36…05d7b` / `541e451e…aef7` |
| USDC vault (Ethereum) | `0xE7A432a28b20621A70E845C7F202004Cf98e7A2C`, about 5,040 USDC backing series S |
| wB24 token | `0x540FDa223a2198690d31920a921A0b9DfF335dbF` |
| Handoff controller | `0x2C7DB4C2fF8514334F62e6630226E95e72Fa6056`; minting and returns both paused |
| Handoff verifier | `0x01d3A6032dF5f892dFa8a1Fc11F04f1c123C985F`, latest checkpoint at network height 1393 |
| Uniswap v4 pool | id `0x70ab0a6f…6cc92c`, wB24/USDC, 0.05% fee, price 1.000002 |
| Pool positions | NFT 515552 (1,905.24 wB24 + 2,000 USDC, centred) and NFT 515553 (3,094.76 wB24, ask side) |
| Operator wallet (Ethereum) | `0xF77B1876803c01967C5a62EF28C5f6de4574F7E1`; about 0.0166 ETH, 0.5 USDC, 1.5 wB24; owns both pool positions |

## What runs, and where

All on the main machine as `systemd --user` units unless noted.

| Unit | Port | What it does |
| --- | --- | --- |
| `pft-wallet-proxy-8080.service` | 8080, 18082 | Wallet proxy and the USDC deposit job service. **Pinned. Do not modify or restart without the founder's say.** |
| `navcoin-relay.service` | 8870, public through a Tailscale funnel on :10000 | The only path from the public Task Node (Fly) to the validators and bridge services. Runs `server/l1/relay.js` from `/home/postfiat/repos/tasknode-navcoin-release`. If it stops, the public wallet stops |
| `navcoin-withdraw.service` | 18084 | Turns a PFUSDC burn into a USDC payout on Ethereum: reads the burn from six validators, builds the proof, sends the payout |
| `navcoin-faucet.service` | 18085 | Sends 0.5 PFT to a new wallet once. Limits: one per address, three addresses per Task Node account, twenty a day |
| `b24-tee-nav-daily.timer` | | Runs the B24 reading at 17:55 UTC for an 18:15 observation |
| Validators | 127.0.0.1:39650 to 39655 | Loopback tunnels to the six validators |
| GPU prover | Vast instance, `ssh -p 20628 root@ssh8.vast.ai` | Rented machine that builds every proof. Lock file `/workspace/b23-gpu/prover.lock` |
| Task Node | Fly app `tasknodeofficial-dev` | Deployed from `pfrpc@100.85.116.26` with `npm run fly:deploy:prod` from a fresh worktree of `origin/main`. Current release v837 |

Unit files are committed under `ops/withdraw/systemd/` on the withdraw branch.

**Writing to the live network.** Every script that writes takes
`/home/postfiat/repos/b23-coord/chain.lock` first, rehearses the operation on a copy of the
ledger, submits, then requires an accepted receipt from all six validators and a matching
full-ledger state (`ops/b24-live/live.py`, `run_operations`). A transaction that commits on
all six validators can still be a rejection; check the receipt code, not only the height.

## What was done on 16 and 17 November

Evidence for each item is under `b23-coord/evidence/` in the folder named.

1. **On-demand NAV reading** (`now/`). `daily.py --now` produces a fresh Schwab reading in
   about 12 minutes for about $8 of cloud spend. B24 moved to proof profile `d5f81a8d…569b42a2`.
2. **USDC withdrawals** (`withdraw/`). The deployed vault's checker was caught up by proof
   and three withdrawals have paid: 0.5, 0.25 and 0.25 USDC. The latest, at network height
   1395, paid in 7 minutes (`0x21bbaf84…718b`).
3. **Pool launch** (`launch/`), in eleven rehearsed steps:
   - 5,025 USDC deposited (`0x009c8320…2f85`) and credited as PFUSDC.
   - 5,000 B24 created for 5,024.61 PFUSDC and exported at heights 1391 to 1393.
   - Receipt proof accepted on Ethereum and 5,000 wB24 minted (`0x16f6c0a3…54ae`).
   - Mint acknowledged on the network at height 1394.
   - Pool seeded, then checked with a real 0.5 USDC swap that matched its quote
     (`0x89800f07…2a3f`).
4. **Public Task Node** (PRs #403, #407, #408). Wallet, deposit and withdraw screens, coin
   pages, starter PFT for new wallets, and a fix that made the coin list one cached request
   instead of 24 sequential reads.

## Known gaps and risks

Ordered by how much they should worry you.

1. **One machine, one operator.** The relay, the withdrawal service, the faucet, the deposit
   job service and the daily reading all run on the main machine. The proofs depend on one
   rented GPU. There is no failover and no alerting. During this work the relay was down for
   about three minutes because of a bad merge in its checkout; nothing detected it but me.
2. **The sixth validator cannot sign Ethereum-side approvals.** Validator 5's authority key
   is wrong. Acknowledgements need five of six, so there is no spare signer. If one more
   fails, mints from the network to Ethereum cannot be acknowledged. Checklist item D5.
3. **The handoff verifier's freshness rule.** A receipt proof must build on the verifier's
   latest checkpoint (now height 1393) and its tip must be within 64 blocks of it. After
   about height 1457 the next export needs `advanceCheckpoint` first. The first mint
   rehearsal failed on exactly this (`UnknownPriorCheckpoint`).
4. **Returns are off.** wB24 cannot be turned back into B24: returns are paused on the
   controller, and minting is paused again after the launch. Someone who buys wB24 in the
   pool can only sell it back to the pool.
5. **Nobody has clicked through the public screens with real funds.** Deposits and
   withdrawals were driven by script through the same services. The deposit screen with
   MetaMask, the withdraw screen and the starter-PFT grant have not been exercised in a
   browser on the public site. Phantom is untested; the code only looks for
   `window.ethereum`.
6. **No swap screen.** The Task Node links to the pool on Uniswap. Whether Uniswap's website
   or MetaMask's swap routes to a new v4 pool for an unlisted token is unverified.
7. **Withdrawals only handle series S PFUSDC**, and only up to what the vault holds for it.
8. **A network bug needed manual repair three times at launch.** No root cause yet. Checklist
   item D4.
9. **The daily reading's first run on the new code** is 2026-11-17 17:55 UTC. Watch it.
10. **Cloud spend.** $80.38 of a $250 cap is used; each reading reserves about $8.
11. **Faucet runway.** The faucet holds about 70 PFT, enough for about 140 new wallets, out
    of a fixed supply of 1,000 PFT. The reserve account holds most of the rest.
12. **Other coins.** A666's pool trades far from its NAV, BMNRC's reading is stale and its
    creation and cash-in should be refusing, and B23 is retired. Checklist section E.
13. **Nothing is merged.** The live validators run a branch. Checklist items C7 and D6.
14. **Stale public text.** The public handoff page and the unpublished blog post (site pull
    request 29, not to be merged until the founder decides) predate the launch.

## The Solana proposal in one paragraph

Two thirds of a deposit's 26 minutes is waiting for Ethereum finality. Solana's Alpenglow
upgrade (mainnet since 2026-10-21) finalizes in under a second and produces a compact BLS
certificate per block, which is the kind of object our existing proof route already checks
for Ethereum. A Solana deposit route would prove "a deposit receipt account was created in a
block this certificate finalizes" in one SP1 proof, admitted by the ledger the same way the
Ethereum proof is. The hard parts are that Solana has no per-account proof, so the proof must
process every account a block changed, and that the validator set changes every epoch with
nothing signing the handover. The spec proposes shipping with the validator set ratified per
epoch by our own validators and replacing that later. A working open-source reference exists
and is cloned at `/home/postfiat/repos/ref-solana/janus`, next to the Solana client source at
`/home/postfiat/repos/ref-solana/agave`. I read the relevant code in both; I have not run
either.

## Suggested first week

1. Read the spec and say whether the governed-anchor first version is acceptable to you.
2. Take checklist items A3, A4, A8 and A9 (how value leaves the Ethereum contracts and who
   holds each privileged function). They are read-only and they are where real money sits.
3. Take D5 (validator 5's key) so that there is a spare signer again.
4. Decide a merge order for the branches (D6), since everything above is easier to review
   once it is on one line.

## Rules that apply to this work

- **Secrets by path, never by content.** Do not print, copy or commit keys, tokens or
  credential files. The Ethereum operator key is at `/home/postfiat/.config/b24/eth-operator.key`.
  The relay credential is in `~/.config/tasknode/navcoin-relay.env`.
  `b23-coord/evidence/tee/b24-daily/20261114/capture/ssh_key` exists; do not read it.
- **Schwab access is read-only.** No orders, no transfers.
- **Nothing moves from the StakeHub custody wallet** without the founder's explicit go, and
  the founder's wallet is never opened or signed with.
- **No consensus change without approval.**
- **One cloud instance at a time**, torn down after use.
- **Do not restart** the pinned wallet proxy on :8080 or the job service on :18082 while the
  founder is testing.
- **Never report something as working without checking it yourself**, and say plainly what
  was not checked.
