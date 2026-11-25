# L1 code and release review, 25 November 2026

Reviewed upstream `main` at `5dd208b8f8049a823008c6f47b130ac7295d7721`. Three P1s affect Ethereum USDC withdrawals after the holder has burned pfUSDC. One has a fix in this change. The review confirmed no P0 and established no historical loss. Determining whether any holder was affected requires the operator's job records and Ethereum receipts.

## What is running

The latest committed fleet observation is 10:16 UTC on 25 November. All six Foundation-operated devnet validators reported revision `04600868`, height 1897 and one state root. Consensus v2 requires five precommits. Cobalt governs changes to validator trust. The operating log records three small Solana USDC payouts on 24 November. For Ethereum, the network status page records the service coming up and older deposits being credited, with no subsequent real withdrawal recorded. The review used the published operator evidence and local fixtures. [Network status](network-status.md) · [Current state](chain-state-current.md)

## P1 findings

### 1. An Ethereum revert can become a paid withdrawal on retry

The sender persists the mined result, including status zero, before signalling failure. On retry, the worker reads the transaction hash from the result file and marks the job paid. In a local replay of the original worker, a saved status-zero receipt produced `stage=paid`. The same file-existence check lets the checkpoint loop advance its local height after a reverted checkpoint. [Sender](https://github.com/postfiatorg/postfiatl1v2/blob/5dd208b8f8049a823008c6f47b130ac7295d7721/ops/withdraw/eth_send.py#L28) · [Payout recovery](https://github.com/postfiatorg/postfiatl1v2/blob/5dd208b8f8049a823008c6f47b130ac7295d7721/ops/withdraw/service.py#L108) · [Checkpoint recovery](https://github.com/postfiatorg/postfiatl1v2/blob/5dd208b8f8049a823008c6f47b130ac7295d7721/ops/withdraw/service.py#L118)

**Fixed in this change.** Fresh and saved results must have a successful mined receipt for the intended action before either state transition. The worker retains rejected evidence for reconciliation. Existing terminal jobs require a separate check. Match each paid job to its Ethereum receipt and the vault's `ProofNativeWithdrawal` event, including the burn commitment, recipient and amount. A status-zero transaction paid gas and transferred no USDC. [Vault event](https://github.com/postfiatorg/postfiatl1v2/blob/5dd208b8f8049a823008c6f47b130ac7295d7721/crates/ethereum-contracts/src/ERC20BridgeVaultL1.sol#L221)

### 2. A worker restart can leave an accepted withdrawal stuck indefinitely

The worker persists intermediate stages but only dispatches queued jobs. Submitting the same burn again only queues a missing or failed job. In a local restart simulation, one worker scan dispatched the queued control and skipped all four intermediate stages: reading, catching up, proving and paying. The readiness endpoint can still report ready because it checks the route and bucket. [Worker scan](https://github.com/postfiatorg/postfiatl1v2/blob/5dd208b8f8049a823008c6f47b130ac7295d7721/ops/withdraw/service.py#L134) · [Resubmission](https://github.com/postfiatorg/postfiatl1v2/blob/5dd208b8f8049a823008c6f47b130ac7295d7721/ops/withdraw/service.py#L162)

**Open.** Persist transaction identity before broadcasting either type of Ethereum call. On startup, resume pre-send work and reconcile possible broadcasts against the chain before deciding whether to send. Exercise restarts on each side of the broadcast and receipt writes. Surface jobs that require operator intervention.

### 3. A normal batched block can strand a valid finalized burn

The payout worker requires the block to have exactly one transaction receipt and always builds a one-leaf Merkle proof. The node commits one leaf per qualifying redemption, so a burn and an unrelated payment can have two receipts but only one exit leaf. Adding an unrelated receipt to the local burn fixture made the original worker reject the burn before proof generation. Retrying cannot change a finalized block. Blocks with multiple redemptions also require a real inclusion path. [Worker check](https://github.com/postfiatorg/postfiatl1v2/blob/5dd208b8f8049a823008c6f47b130ac7295d7721/ops/withdraw/service.py#L106) · [Witness](https://github.com/postfiatorg/postfiatl1v2/blob/5dd208b8f8049a823008c6f47b130ac7295d7721/ops/withdraw/service.py#L125) · [Canonical leaves](https://github.com/postfiatorg/postfiatl1v2/blob/5dd208b8f8049a823008c6f47b130ac7295d7721/crates/node/src/batch_snapshot.rs#L252)

**Open.** Export the canonical ordered exit leaves and use the protocol inclusion-proof builder for the selected withdrawal. Verify the witness against the finalized exit root for a burn with an unrelated payment and for each of two burns in one block. Also cover paying the second claimant after the first has advanced the Ethereum checkpoint. [Inclusion-proof builder](https://github.com/postfiatorg/postfiatl1v2/blob/5dd208b8f8049a823008c6f47b130ac7295d7721/crates/types/src/pfusdc_tier4_types.rs#L1799)

## Release qualification

The PFT lane-export identity workflow failed on the reviewed commit during the guest build/key derivation step. The open repair describes stale guest and prover lockfiles under the locked build. This run did not reach identity comparison. A successful clean build and comparison against the pinned identity are still required for that lane. [Failed run](https://github.com/postfiatorg/postfiatl1v2/actions/runs/45669706235) · [Repair PR #216](https://github.com/postfiatorg/postfiatl1v2/pull/216)

The historical productionization closure-table check passes all 35 entries. The separate public-testnet inventory records 4 gates done, 16 open and 1 unknown. Open gates include validator key custody, independent recovery evidence, specialist reviews and exact-candidate release verification. The current handoff also flags a private governance item for operator review before the validator move. Its underlying evidence was outside this review. [Release gate inventory](release-gate-inventory.md) · [Operator handoff](handoff-2026-11-25.md)

## Coverage and verification

| Area | Review and local results |
| --- | --- |
| Ordering, governance and storage | Traced persistence before signing, vote guards, history indexing and recovery. Rust library suites passed for fast ordering (35), Cobalt (88), storage (105 with two ignored), execution (301 with two ignored), types (272), NAV reserve (1) and privacy (5). Python storage scaling and account-history checks passed (144). |
| Bridge and operations | Reviewed the Ethereum worker, sender and vault event, the node's exit-root construction, Solana broadcast recovery and settlement quorum, and B24 release selection. Ethereum withdrawal suite passed (11), Solana recovery and quorum passed (31), and B24 daily passed (42). |
| Wallet and RPC | Wallet web passed all 290 tests. Wallet proxy withdrawal-job, public-read-cache and connection-lifecycle regressions passed. Python persistent RPC client passed 23 tests. RPC inventory covered 170 methods. |
| Release and proofs | Ran the consensus determinism surface, proof public-input inventory and historical closure-table checks. Inspected the hosted identity run separately. The proof inventory also reports previously documented pfETH guest source drift. |

The Ethereum regressions run with `python -m unittest discover -s ops/withdraw` after installing the repository's test requirements. They use a temporary job store and stub external reads and sends. This review did not run a full workspace release battery, a cryptographic audit, a fleet restart, a paid proof or a live transfer.
