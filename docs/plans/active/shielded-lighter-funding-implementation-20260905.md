# Lighter funding implementation

Task Node: `task_1b81da68342923f814770ea149d3f048` (accepted; reward unclaimed).
Design: [locked Lighter extension](../../specs/shielded-lighter-funding-hardening-20260905.md).
The separate [Hyperliquid implementation](shielded-hyperliquid-cover-implementation-20260905.md)
and [configurable denomination amendment](../../specs/shielded-funding-configurable-denomination-20260905.md)
retain the qualified-cover requirements and permit the small functional pilot.
Code paths below are in sibling StakeHub.

## Current result — September 6

The authorized 0.005 ETH route completed from the existing StakeHub source,
through shielded ingress/private exit and finalized Ethereum release, to an
exact **0.005 ETH spot credit on Lighter**. The account's authenticated history
matches the original finalized L1 transaction. Margin remains disabled; this
pilot does not establish margin or trading readiness. No CEX was used.

CLI state and the loopback interface report completion. This remains
`functional-unqualified`; same-owner accounts and a thin pool do not establish
independent participation or demonstrate anonymity. Live identities, exact
transaction evidence and recovery keys are retained privately.

## Implemented and verified

- [x] Research lock: first compliant passing full gate 86.47/100 after 85.00 and direct-model revision; [lock evidence](../../status/shielded-lighter-hardening-20260905/lock.json).
- [x] Venue-conditioned inference retains other-venue spend constraints without borrowing Hyperliquid destinations: `stakehub/shielded_exit_privacy.py`.
- [x] Exact configurable integer principal through native proofs and Lighter deposit; no implicit 1 ETH requirement. Denomination-specific inference cannot borrow other amounts.
- [x] Complete functional source deposit/mint/shield/private-exit/burn/release controller: `shielded_exit_funding.py`, `shielded_exit_runtime.py`. Lighter bypasses Arbitrum.
- [x] Reviewed/pinned deployed route and settlement implementations; original live SP1 ingress and egress proofs accepted by the deployed verifier.
- [x] Native CPU egress proof completed in about 24 minutes with 43.2 GiB peak memory and no swap, preserving the original burn and finalized checkpoint.
- [x] Exact Ethereum release finalized; native settlement passed six-validator agreement and full local replay through block 992.
- [x] Signed sponsored Ethereum venue batch, exact deposit event/account binding and authenticated credit: `shielded_exit_lighter_settlement.py`, `shielded_exit_lighter_auth.py`.
- [x] Fresh-account and unused-key API responses are distinguished narrowly from failures. Original owner-controlled API key registration is retained across retries; missing history never triggers another deposit.
- [x] Shared sponsor stages retain chain-specific nonce reservations through finality; rebroadcast races reconcile the original hash, retaining signed bytes.
- [x] Python CLI and working loopback interface show saved jobs, exact amounts, account destinations, status and fixed-job resume actions. Foreign origins, invalid CSRF and arbitrary browser arguments are rejected.
- [x] Exact authenticated 0.005 ETH spot credit verified after Ethereum finality. Driver completed at 05:55 UTC; margin disabled.
- [x] Combined shielded-funding/dashboard tests: 388 passed, four native opt-ins skipped. Actual live proofs and full native replay are independent evidence. Strict StakeHub documentation build passed.

## Remaining qualified-cover gates

- [ ] Complete finalized cross-chain history and funded Lighter cover qualification, including the separate operator-aware result and later-disclosure sensitivity.
- [ ] Cover-operator inventory, finite venue holding capacity and measured sustained operating budgets.
- [ ] Task Node initial evidence, requested verification and rewarded outcome.
- [ ] Retire the broader milestone when its required qualified functionality and Task Node lifecycle are complete.

The shared replay, profile and resource-limit repairs are summarized in the
Hyperliquid milestone. Existing paid deposits, burns, signing identities and
journals were preserved. The small functional pilot does not close these
remaining cover-service gates.
