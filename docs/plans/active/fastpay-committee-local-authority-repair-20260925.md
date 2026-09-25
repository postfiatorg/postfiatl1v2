# FastPay committee authority repair

Specification: [locked repair specification](../../specs/fastpay-committee-local-authority-repair-20260925.md).
The specification bytes are locked, including their original proposed-status line.
SHA-256: `759ca8e44816b44e3737cb8cf7635b8d77d24273b4d1d6eac4e23809b8ab8a2f`.

- [x] Full TIH gate, `round-20260925T013542Z`: five scores each from
  `gpt-6-astra` (92.40), `anthropic/claude-fable-5.1` (80.80),
  `z-ai/glm-5.3` (89.60). Combined **87.60/100**; locked at first passing round.
- [x] Confirm six live hosts run the same deployed binary and registry; all at
  height 1031 and reject recovery capabilities with the same committee mismatch.
- [ ] Replace global registry equality with local signer authorization and verify
  actual signatures before durable mutation in `crates/node/src/fastpay_recovery_node.rs`.
- [ ] Exercise six-member key rotation, negative cases and certified convergence
  in `crates/node/src/tests/fastpay_payment_safety.rs`; run affected client tests.
- [ ] Build minimal repair from deployed revision `707e006f`; qualify isolated
  current-state snapshots and preserve signed deployment/rollback bindings.
- [ ] Roll all six hosts and verify capabilities, Python CLI payment, wallet UI
  result and certified convergence; publish exact evidence and operator guidance.

Task Node's skill/integration is unavailable in this session. No generated task,
acceptance, submitted evidence or reward is claimed. User-authorized repair work
continues, with the lifecycle gap recorded here rather than invented receipts.

Operational limit: five eligible FastPay signers remain; one further signer outage
prevents new FastPay certificates until a governed committee rotation.
