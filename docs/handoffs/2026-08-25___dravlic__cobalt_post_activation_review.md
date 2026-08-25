# Cobalt post-activation review

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-08-25 UTC

## BLUF

Took over after the 2026-08-25 Cobalt activation run. Confirmed from the repository and the packet verifier that Cobalt has been the live validator-trust authority on the controlled testnet since height 916, that the first Cobalt-authorized key rotation committed at height 917, and that no active plan remains. Two published documents now lag the live state: the earlier handoff still says deployment stopped before live mutation, and the blog still says Cobalt authority is off. Drafted the proposed next research specification, [Cobalt Adversarial Verification](../governance/cobalt-adversarial-verification-research-spec.md), which turns the open critique of the activation evidence into experiments. Nothing is committed, scored, or requested from Task Node yet.

## Current state

- Live boundary, as recorded in [`activation-status.json`](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/cobalt-activation-live/packet/activation-status.json): status `ACTIVATED`, authority mode `cobalt-validator-trust`, `writes_validator_registry: true`, `controls_block_consensus: false`, chain `postfiat-wan-devnet-2`, height 919, registry root `945768d5…c37e`.
- Verified locally: `python3 benchmarks/cobalt-activation-live/packet/verify_packet.py` returns `packet-ok` with root `b603b59d0245a7c73e766d0ba7fb19975f11e1e39bdd7263bf87e65250438bfb`; `docs/plans/active/` is empty; worktree clean at `fe046536` on `main`.
- Task Node: 22 rewarded, 21 refused, 0 outstanding on the shared `0xPostFiatChad` link. Live cutover task `task_40f389235be48756a9933f42d0d4dc6c` is rewarded.
- Stale documents:
  - [`2026-08-25___postfiatchad__cobalt_governance_activation.md`](2026-08-25___postfiatchad__cobalt_governance_activation.md) body still reads "Deployment was deliberately stopped before live mutation" and lists the cutover as the next action; only its links were updated after the cutover.
  - `postfiatorg.github.io/content/blog/cobalt-further-evaluation.md` line 154: "Cobalt authority remains off today".
- Not re-probed on the live fleet from this session: node processes, heights, and roots come from the committed packet, not from a fresh `live-status` run.

## Review of the activation evidence

Grounded in the locked specification, the completed milestone, and the article. Detail and the experiment mapping are in the draft specification's "Claims, evidence, and gaps" table.

1. **Scope wording.** The code boundary is correct: Cobalt ratifies registry and trust-graph changes and never touches block finality. The public phrase "validator governance" reads as the whole governance. Cobalt does not decide who deserves trust; that is a separate layer.
2. **Proposal source is still central.** Every live change so far was proposed by the Foundation operator and authorized by six Foundation-administered validators. Cobalt gates changes; it does not yet decentralize who asks for them.
3. **A locked gate was redefined.** The activation specification's Experiment 4 and ACTIVATE gate required at least three independently controlled operator groups, and stated that no milestone may redefine the gates without a newly locked specification. The milestone cancelled the operator task and re-scoped independence to simulation. The activation went ahead on that basis. This needs a recorded decision, not silence.
4. **Cooperative evidence only.** Faults were scheduled, not adversarial; one Byzantine strategy; oracle and corpus by the implementing team; 18 cases; 50 finality rounds. The blog acknowledges the limits. The next specification is where they get closed.
5. **Reversibility is Foundation-controlled.** Rollback was rehearsed on a clone and is authorized by the same six validators. Fine for a testnet, should be stated as such and executed live once.

## Next decision or action

Decisions taken by the second operator; object in the next handoff if any is wrong:

1. **Next specification: adversarial verification.** The activation handoff itself names thorough adversarial testing as the open work, and no active plan exists.
2. **Independent-operator gate: decided inside the specification.** Experiment 6 records whether the gate from the locked activation specification is reinstated as its own milestone or formally deferred, and that outcome is locked with the rest of the specification. It is not silently dropped.
3. **Article correction now.** "Cobalt authority remains off today" is a factual error about the live state and is corrected ahead of the adversarial results.
4. **Task Node identity: the shared `0xPostFiatChad` link on this machine.** The machine and the link were provided for this purpose; the operator name in handoffs and commit messages identifies the human.

Next bounded actions, in order: score the draft with the Text Improvement Harness, lock it through Task Node, build the milestone document through Task Node, then run the experiments as Task Node tasks.

## References

- Draft specification: [`cobalt-adversarial-verification-research-spec.md`](../governance/cobalt-adversarial-verification-research-spec.md)
- Locked activation specification: [`cobalt-activate-or-retire-research-spec.md`](../governance/cobalt-activate-or-retire-research-spec.md)
- Completed milestone: [`cobalt-activate-or-retire-milestone.md`](../plans/completed/cobalt-activate-or-retire-milestone.md)
- Activation packet: [`benchmarks/cobalt-activation-live/packet`](https://github.com/postfiatorg/postfiatl1v2/tree/main/benchmarks/cobalt-activation-live/packet)
- Article: [Cobalt: Further Evaluation](https://postfiat.org/blog/cobalt-further-evaluation/)
- Prior handoff: [`2026-08-25___postfiatchad__cobalt_governance_activation.md`](2026-08-25___postfiatchad__cobalt_governance_activation.md)

## End of session 2026-08-25

- Commits: `3d1c2ab4` added this handoff and the draft adversarial-verification specification; `e7d1fe4` corrected the Cobalt authority state in `postfiatorg.github.io`; `a6aaa4bb` locked the specification; `683eb3be` added the milestone document.
- Text Improvement Harness: 89.27 average (GPT 89.20, Fable 88.40, GLM 90.20).
- Rewarded Task Node tasks: specification lock `task_158622307482e23fb4519889b53b475f`; milestone document `task_d28eb3465dcac9a32524c25bba996e1e`.
- Current state: E1 is being started as a `/goal` run in tmux session `dravlic` on this machine and will update the milestone document as it progresses. E2 will not start automatically.
