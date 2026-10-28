# Base Sepolia token live, lock contract merged and exchange page corrected

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-10-28 UTC

## BLUF

The [exchange guidance page][exchange] was corrected after the 27 October review and republished at 10:41 UTC. The checker found 0 problems in 155 references, and the harness score went from 87.33 to 87.40. Its sources are branch `site-source` of the private repo `postfiatorg/exchange-guidance-review` at `289af79` (its `HANDOFF.md` was updated at `b6439d9`) and public site commit `2676ddf`.

The PFT token was deployed on the Base Sepolia test network (chain id 84532) at 10:24 UTC. It is a test deployment with no value. The token is `0x18fE4ca51A0cE042688D5A54917319d03B527D92`. The treasury Safe is `0x7424F4F4A620d37CFB54A4558a1Ec7f60b71A0CB`: two owners, threshold 2, no module and no guard, created through the canonical factory. The supply is 100 billion PFT, a test figure. The source is verified on Sourcify and Blockscout, and the [token record][tokenrecord] is in `b3ce24f9`. Milestone item P1.3 is ticked.

The Phase 2 lock contract `PFTLockV1` was written, reviewed and merged in `39bfb547` with 111 tests, and P2.3 is ticked. Its deployment script was merged in `182b3f22` after its own review, with `test/deploy-lock-cli.sh` running in CI. The script was rehearsed on Base Sepolia at 13:00 UTC: lock `0xE36e7Fc384564419187546c19719A5654266dE0B` against the test token, with a cap of 2 billion PFT and the reverting stand-in verifier `0x531659D05d86C5C4cE6fdc3C473b246C294f2Ec9`. Its source is verified, and its [record][lockrecord] is in `0204a986` with mode `test`.

The token's deploy script went through three more correctness rounds (`d1d06080`, `72309224`, `db7ca922`, `c46f1cdb`, `2b21aa4e`, `b4f98c72`), with three results:

- the treasury Safe's creation is verified from its signed creation transaction, bound to the deployment chain;
- the record's compiler settings and code hashes are bound to the deployed code;
- a guard is refused unless named.

The product owner made four decisions on 2026-10-28:

- the total supply is fixed at 100 billion;
- the target is late Q1 2027 on Base, with an SPL version on Solana;
- there is a five-year monthly escrow;
- the Base ERC-20 is a slice of the 100 billion.

They are recorded in the Coinbase texts, the [milestone][milestone] and the [decision sheet][decisions] (`093543db`, `21f0da62`, `42f5fe40`).

## Current state

**Exchange guidance page.** It is live with the corrections. The next biweekly update is due 2026-11-10, per the review repo's `HANDOFF.md`. The Q4 item "Base Sepolia deployment" is now done and can be reflected then. The Playwright tests were not run, because the build host has no browser. Instead, the live file was decrypted and compared.

**Token on Base Sepolia.** The values are in the BLUF and the [dry-run record][dryrun]. The record predates the fields added in rounds 5 and 6 (the creation transaction and both code hashes). The deployer `0xDEe78dB2bC6f5EFCcAB7A046997E5c413C26316D` is retired at nonce 2. The funding wallet `0xC75Bf05Ce82d6f4b6139dd9446D6De5F5994a4CB` holds about 0.005 Base Sepolia ETH.

**Token deploy script** ([`DeployPFTToken.s.sol`][tokenscript]). It has 129 tests at 100 % line coverage, plus the CLI test with 11 anvil cases. The procedure is in the [token README][tokenreadme]. The public RPC at sepolia.base.org does not serve `eth_getRawTransactionByHash`, so `PFT_TOKEN_TREASURY_CREATION_RAW_TX` (from `cast tx <hash> --raw`) is required there.

**Lock contract** ([`PFTLockV1.sol`][lock]).

- It is merged (`39bfb547`), with the deterministic accounting and permit tests (`20eabfcf`, `87fcdc20`).
- Its deployment script [`DeployPFTLock.s.sol`][lockscript] is merged with `test/deploy-lock-cli.sh` (`182b3f22`). The README's four broadcast outcomes are in `4b445c69`.
- In the Base Sepolia rehearsal, nothing can be released from the test lock, because the stand-in verifier reverts on every call. The end-to-end lock was not exercised, because the only token holder is the 2-of-2 Safe.
- The real Base deployment waits for three things: the PFTL finality verifier on Base, the genesis cap and the audit.

**Python.** The state reader [`pft_token.py`][pyreader] reads the Sepolia token live. [`pft_token_ui.py`][pyui] is a read-only page. Both landed in `4bd5744a` and `ad61ff6a`, M2 and M3 are ticked, and the usage is in [RPC examples][rpcexamples].

**Design note** ([Phase 2 design note][phase2]).

- Invariant 4 is restated net of releases (`5d002069`).
- The release interface and the export recipient rule came in through the merge (`20eabfcf`).
- The cap's immediate lowering, its seven-window raise delay, the false-release watcher and the response are in `43394aba` and `a83f1973`.
- The harness score is 84.02, so the note is not locked.

**Options paper.** The [options paper][paper] was rewritten as one document, revised 2026-10-28 (`9cde4a6b`, after `eb8c5bd3`). Its harness score is 84.00, so it is not locked either. The [milestone][milestone], which is this lane's journal, records the judges' remaining points on both documents and the design note's stale line references into the paper.

**Weekly scorer (Z2 step 6).** Round 29 was counted and clean: 0 flips, an overlap of 25/25 and the control OK (`eb29b5cf`, [weekly record][weekly]). This is a shadow evaluation and publishes nothing. The next counted rounds are 11-03, 11-10 and 11-17.

**Bot PRs.** #167, #170, #171, #174 and #175 are merged (`8c6ec6db`, `ff61beb6`, `d8513ba2`, `924a3fa6`, `72475b01`). The two vote-record follow-ups (#174, #175) are release candidates, together with the VR-01 fix.

**Dependencies.** k256 is removed from the node (`6f7aae33`), and the `deny.toml` reason is restated. The root lock still lists crypto-bigint 0.5.5, through alloy-primitives' optional k256. The k256 0.14 route is blocked upstream by Alloy, SP1, near-vm-runner and dalek-ff-group-mirror. The Solana stake reader's two-package lock update is recorded in the [next release plan][next] for its own run.

**Devnet and deployment state.** The devnet was not touched today: no deploy and no governance action.

- The last observed fleet state is still the 2026-10-27 12:14Z after state of `index-v3-20261027` (source `d728f895`) ([Current State][state], [previous handoff][previous]). No live fleet probe was made in this session.
- Main was at `1c6324a4` before this handoff.
- Merged but not deployed: the VR-01 fix `5fc43961`, the vote-record follow-ups and the k256 removal.
- The only live reads today were of Base Sepolia.

**CI.**

- docs-build is green on every commit.
- rust-ci is green on `72309224`, `21f0da62`, `42f5fe40` and `c46f1cdb`; later runs were still running.
- product-security-ci is green up to `42f5fe40` and red from `c46f1cdb`. The deploy-script CLI test switched a running anvil's chain id, which the workflow's Foundry v1.3.6 refuses for its signer. The fix is `1c6324a4`: two anvils, reproduced on the older Foundry. Its CI run is still to be read.

**Task Node.** Tasks were requested, accepted and rewarded, in PFT:

| Work | PFT |
|---|---|
| Sepolia token deployment | 4 |
| lock contract | rewarded |
| lock merge | 2 |
| lock rehearsal | 3 |
| Python reader | 3 |
| Python page | 2.8 |
| scorer round | 2 |
| deploy-script rounds | 3, 3, 2.45, 3 |
| dependency removal | 1.4 |
| bot PR batch | awaiting review |

Two generation failures were recorded as skips.

## Next decision or action

My next steps, in order:

1. Cut the next validator release (the VR-01 fix `5fc43961` plus the two vote-record follow-ups) once rust-ci and product-security-ci are green on the chosen commit, then roll it out.
2. Fix the design note's stale line references into the paper and address the judges' remaining content points. Both documents are still below the 86 gate.
3. Do the Solana stake reader's lock update in its own run.
4. Build the Phase 2 genesis lane and release path once the mainnet genesis plan exists.
5. Publish the biweekly page update on 2026-11-10: the Base Sepolia token and lock rehearsal are done, and the lock contract is merged.

Only your lane can provide these. Each shows the date it was first asked:

- the ERC-20 slice size, the launch allocation, the issuing entity, the treasury Safe's signers and threshold, the treatment of PFT already paid on the PFT Ledger, and the audit budget and vendor (2026-10-26; the total supply was answered 2026-10-28; all on the [decision sheet][decisions]);
- the two issuer signatures per the [issuer signing sheet][issuer], A666 first, then BMNRC (2026-09-25 and 2026-10-01);
- the on-chain registration of the reserve-proof successor (2026-09-24);
- the `ETHEREUM_MAINNET_RPC_URL` secret (2026-09-28);
- the StakeHub acceptance file and binaries (2026-10-21);
- SCT-08 items 1–3 ([campaign][sct08], 2026-10-23);
- the production genesis plan and the website go-live (2026-10-23).

## References

- [Exchange guidance page][exchange]; private repo `postfiatorg/exchange-guidance-review`: `HANDOFF.md` on main at `b6439d9`, its PR #1, branch `site-source` at `289af79`; public site commit `2676ddf`
- [Milestone][milestone], [dry-run record][dryrun], [token record][tokenrecord], [lock record][lockrecord], [token README][tokenreadme], [`DeployPFTToken.s.sol`][tokenscript], [`PFTLockV1.sol`][lock], [`DeployPFTLock.s.sol`][lockscript]
- [Phase 2 design note][phase2], [options paper][paper], [decision sheet][decisions]
- [`pft_token.py`][pyreader], [`pft_token_ui.py`][pyui], [RPC examples][rpcexamples]
- [Weekly scorer record][weekly], [next release plan][next], [Current State][state], [issuer signing sheet][issuer], [SCT-08 campaign][sct08]
- My [previous handoff][previous]
- Main: `b3ce24f9`, `39bfb547`, `20eabfcf`, `87fcdc20`, `182b3f22`, `4b445c69`, `0204a986`, `d1d06080`, `72309224`, `db7ca922`, `c46f1cdb`, `2b21aa4e`, `b4f98c72`, `1c6324a4`, `093543db`, `21f0da62`, `42f5fe40`, `4bd5744a`, `ad61ff6a`, `5d002069`, `43394aba`, `a83f1973`, `9cde4a6b`, `eb8c5bd3`, `eb29b5cf`, `8c6ec6db`, `ff61beb6`, `d8513ba2`, `924a3fa6`, `72475b01`, `6f7aae33`, `5fc43961`

[exchange]: https://postfiat.org/exchange-integration/
[previous]: 2026-10-27___dravlic__index_v3_release_live_and_token_deploy_procedure_hardened.md
[state]: ../status/chain-state-current.md
[milestone]: ../plans/active/pft-erc20-base-listing-milestone.md
[next]: ../plans/active/next-validator-release-plan-20261108.md
[dryrun]: ../listings/coinbase/pft-token-dry-run-20261026.md
[paper]: ../listings/coinbase/pft-erc20-representation-options-20261026.md
[phase2]: ../specs/pft-erc20-base-bridge-phase2-design-20261026.md
[decisions]: ../governance/pending-operator-decisions.md
[weekly]: ../governance/dunl-subscorer-shadow-weekly-2026.md
[rpcexamples]: ../rpc/examples.md
[sct08]: ../review/sct08-evidence-campaign-20261023.md
[tokenrecord]: https://github.com/postfiatorg/postfiatl1v2/blob/1c6324a4/crates/pft-token-contracts/deployments/pft-token-base-84532/deployment.json
[lockrecord]: https://github.com/postfiatorg/postfiatl1v2/blob/1c6324a4/crates/pft-token-contracts/deployments/pft-lock-base-84532/deployment.json
[tokenreadme]: https://github.com/postfiatorg/postfiatl1v2/blob/1c6324a4/crates/pft-token-contracts/README.md
[tokenscript]: https://github.com/postfiatorg/postfiatl1v2/blob/1c6324a4/crates/pft-token-contracts/script/DeployPFTToken.s.sol
[lock]: https://github.com/postfiatorg/postfiatl1v2/blob/1c6324a4/crates/pft-token-contracts/src/PFTLockV1.sol
[lockscript]: https://github.com/postfiatorg/postfiatl1v2/blob/1c6324a4/crates/pft-token-contracts/script/DeployPFTLock.s.sol
[pyreader]: https://github.com/postfiatorg/postfiatl1v2/blob/1c6324a4/python/postfiat_rpc/pft_token.py
[pyui]: https://github.com/postfiatorg/postfiatl1v2/blob/1c6324a4/python/postfiat_rpc/pft_token_ui.py
[issuer]: https://github.com/postfiatorg/postfiatl1v2/blob/61e9b00c8399907f45dff2be1ee44b8f1da431af/deployments/fastlane-cap-20261014/ISSUER-SIGNING-SHEET-20261020.md
