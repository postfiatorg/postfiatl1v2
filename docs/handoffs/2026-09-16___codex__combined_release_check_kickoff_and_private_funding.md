# Combined release checks and private funding

- **Operator:** Codex (`codex`)
- **Date:** 2026-09-16 UTC

## BLUF

**The existing fixes are already combined in [L1 PR #41](https://github.com/postfiatorg/postfiatl1v2/pull/41). The resumed software suite passed: 1,433 tests, zero failures, 39 ignored. All six saved nodes passed full-history verification.** The final independent rollback replay also passed. All three technical repairs now have passing local qualification evidence. Follow the [repair milestone](../plans/release-blocker-repairs-20260916.md) and the qualification packet at `deployments/release-repair-20260916/`.

The newer private Hyperliquid funding work is saved on separate private branches. It still needs integration and completion.

## Current state

The memory failure interrupted qualification. Codex resumed it at 02:39 UTC using disk-backed temporary files, two workers and memory limits. The six history checks completed by 02:58. The full suite, isolated latency test, formatting, compiler checks and Clippy have now passed. No test was removed or weakened.

The active checkout is:

```text
/home/postfiatchad/repos/postfiatl1v2-release-20260915
branch: release/combined-devnet-20260915
qualified node source: 1c435f4fb482ea7830bee5c7018d370a10a34bfd
```

This source includes both exact PR #37/#39 tips and all seven queued consensus fixes. There is no outstanding choice between #37 and #39 for this candidate.

| Check | Result |
|---|---|
| Full history on six saved nodes | PASS through block 1021, including the original supply failure at 1011 and the local governance change |
| Six-node governance installation and restart | PASS; all six accepted the change and recovered the same certified tip |
| Two node builds | Identical executable hashes from clean source trees and separate existing target caches |
| Arc and pfETH withdrawal proof builds | Both reproduced their existing executable and verification-key identities; downloaded artifacts independently checked |
| Full software suite and remaining software gates | PASS; the separate latency check used the already-built workspace test executable |
| Pre-activation rollback | PASS: exact old executable checkpoint/restart on six restored copies, paired with final-source full replay of restored validator-0 at block 1020 |

The proof CI ran at `5b1093fa`; its workflow and historical release inputs are unchanged at the qualified node source. The only subsequent production-code change canonicalizes retained FastPay vote order and rejects duplicate validators.

The old executable still has its original full-replay bug. The rollback method therefore pairs its checkpoint verification and restart with the repaired verifier's full-history check of the same original state. It requires compatible pre-upgrade raw data. After a live V2 installation, recovery requires compatible software rather than restoring pre-activation state.

All exercises used disposable local copies. This session performed no fresh live-fleet probe, deployment, validator restart, governance activation or transfer of funds. PR #41 remains a draft. The [original failed assessment](https://github.com/postfiatorg/postfiatl1v2/blob/12721f618999dcb175ff4e4397743ece0682220b/deployments/combined-release-20260915/README.md) remains intact.

## What private Hyperliquid funding means

The intended product starts with ETH, moves value through PostFiat's shielded system, funds the user's intended Hyperliquid account, and supports returning the money or recovering safely after an interruption:

```text
ETH → WETH-backed pfETH → shielded transfers/reshaping
    → withdraw WETH → Arbitrum → USDC → intended Hyperliquid account
```

The private portion uses pfETH; Hyperliquid receives USDC. The privacy objective is to reduce the connection between the original funding source and the final account. Public withdrawal amounts and timing remain visible. A successful ordinary deposit does not establish privacy.

The product also needs protected signing, pending-transaction and fee accounting, duplicate-payment prevention, return/re-shielding, and recovery onto another machine. Its intended interfaces are a Python CLI and private browser application.

| Pull request | Relevant work |
|---|---|
| [L1 #37](https://github.com/postfiatorg/postfiatl1v2/pull/37) | Arc/USDC proof and bridge integration; included in #41 |
| [L1 #39](https://github.com/postfiatorg/postfiatl1v2/pull/39) | NAVCoin external routes, pfETH foundations and replay repairs; included in #41 |
| [StakeHub #8](https://github.com/postfiatorg/StakeHub/pull/8) | Companion CLI/dashboard, signing/deposit/checkpoint tools and retained Hyperliquid/Lighter modules |
| [L1 #41](https://github.com/postfiatorg/postfiatl1v2/pull/41) | Combined L1 foundations, accumulated fixes and compatibility repairs being qualified here |

The completed route described by #39/#8 was an Ethereum USDC/NAVCoin/Uniswap round trip. None of these PRs establishes delivery of the complete private Hyperliquid product.

The newer work is on these **private recovery branches**:

- [L1 private archive](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/tree/recovery/private-funding-20260910), checked at `fcf9820a`.
- [StakeHub](https://github.com/postfiatorg/StakeHub/tree/recovery/private-funding-20260910), checked at `4c503264`.

Recorded partial return/accounting tests cover 27 native scenarios and 91 regressions with synthetic external responses. They establish component progress, not live delivery.

## Next decision or action

1. Publish the passing repair evidence and update PR #41. Complete verification of existing Task Node task `task_0747d04c6cffb93874ae122c17faeae6`; no competing task or specification is needed.
2. Review the combined candidate. The original packet's seven reachable-history scan findings, unavailable pinned testnet archive state, and live activation/rollback decision remain separate release obligations.
3. Reconcile the newer recovery branches privately with the qualified L1 candidate. Preserve the 23 local StakeHub safety changes in `/home/postfiatchad/repos/StakeHub-safety-20260907`. The preliminary merge preview found seven conflicting L1 files and 25 conflicting StakeHub files.
4. Continue the private-funding [existing completion contract](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/blob/fcf9820a63b90de6cd351fb7f16e1d449813b251/docs/plans/active/eth-private-funding-completion-contract-20260913.md), starting with **A4: inventory the original worker and offline exports**. Remaining work includes full ETH-entry/return composition, whole-job accounting, exclusive restore, installed CLI/browser qualification and approved real funding/private-funding/return/recovery demonstrations.

A fifth review and the minor-bug sweep are follow-up work. The earlier timing-test flakiness still deserves a deterministic fix; passing this run does not erase that history.

## References

- Local qualification progress: `~/.cache/release-repair-20260916/resumed-qualification/status.json` and `completion-status.json`.
- [Locked repair specification](../specs/release-history-compatibility-20260916.md).
- [Private-funding execution handoff](https://github.com/postfiatorg/postfiatl1v2-private-archive-20260717/blob/fcf9820a63b90de6cd351fb7f16e1d449813b251/docs/handoffs/2026-09-15___codex__eth_private_funding_execution_handoff.md).
- Machine-wide map: `/home/postfiatchad/INTEGRATION-START-HERE.md`.
- Verified local backups and merge previews: `/home/postfiatchad/archive/integration-map-20260916/`. The pasted backup handoff's `/home/postfiat` paths refer to another machine.
