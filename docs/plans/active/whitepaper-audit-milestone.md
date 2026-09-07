# Whitepaper alignment audit milestone

Governed by Task Node audit `task_dae56e5e3650cf674495617e5547f747` and prerequisite `task_e8a5201ae49adfd0e32b5496bb7f7188`. Source baseline: `d351353e57b295368450a57866ace17b5e1ce6ad`.

The locked specification is `docs/specs/whitepaper-alignment-audit-20260907.md`; lock SHA-256 `4b683479a1d82fd1ef387797a5cf5bd751b53f75063b8985ec15441441f914c0`, first full gate **87.00/100**. Lock record: `docs/review/whitepaper-audit-lock-20260907.md`.

- [x] Accept both generated Task Node tasks; isolate the worktree and pin the original whitepaper hash.
- [x] Read the complete paper, identify the canonical candidate, and lock the research specification.
- [ ] Trace all sections and Appendix A claims to code, tests, documents, or explicit evidence gaps. Start at `crates/node/src/cobalt_handoff.rs`, `crates/ordering_fast/src/consensus_v2.rs`, `crates/node/src/orchard_state_application.rs`, and `crates/consensus_cobalt/src/validator_admission_policy.rs`.
- [ ] Publish the overview, structured claim inventory, generated alignment table, and prioritized gap backlog under `docs/architecture/`.
- [ ] Correct verified drift in the whitepaper, raw download, README, STATUS, and relevant architecture/governance/privacy/evidence pages. Preserve protocol behavior and historical observation dates.
- [ ] Implement the offline Python inventory CLI at `python/postfiat_rpc/whitepaper_audit.py`; verify invalid inventory, anchors, coverage, section filtering, and JSON output.
- [ ] Expose the same inventory through MkDocs navigation; validate the rendered table and inspect the reader-facing pages.
- [ ] Run proportional existing protocol tests plus documentation, link, redaction, and strict-build checks; record exact results and omissions.
- [ ] Package durable review artifacts and satisfy Task Node verification for both tasks.
- [ ] Move this milestone into completed plans once the CLI, documentation interface, validation, and task evidence are complete.

Runtime semantics, validator configuration, live fleet operation, signing, and value movement are outside this milestone.
