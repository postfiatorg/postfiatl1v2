//! Explicit-state Packet-P0 model for FastSwapV1.
//!
//! This is deliberately independent from validator RPC and storage code.  It
//! models persisted facts only: if a vote bit is visible, the corresponding
//! reservation/high-water/lock/terminal fact is already durable.

use std::collections::{BTreeMap, VecDeque};

pub const MAX_VALIDATORS: usize = 6;
pub const SWAPS: usize = 2;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Decision {
    Confirm,
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ModelConfig {
    pub validator_count: u8,
    pub max_depth: u8,
    pub enforce_stale_qc_guard: bool,
}

impl ModelConfig {
    pub fn validate(self) -> Result<Self, ModelError> {
        if !(4..=MAX_VALIDATORS as u8).contains(&self.validator_count) {
            return Err(ModelError::InvalidValidatorCount(self.validator_count));
        }
        if self.max_depth == 0 {
            return Err(ModelError::InvalidDepth);
        }
        Ok(self)
    }

    pub fn fault_tolerance(self) -> u8 {
        (self.validator_count - 1) / 3
    }

    pub fn quorum(self) -> u8 {
        (2 * self.validator_count) / 3 + 1
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelError {
    InvalidValidatorCount(u8),
    InvalidDepth,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ValidatorState {
    /// 0 = no reservation, 1 = swap A, 2 = swap B.
    pub reservation: u8,
    pub highest_precommit_round: [u8; SWAPS],
    /// 0 = unlocked, 1 = confirm, 2 = cancel.
    pub decision_lock: [u8; SWAPS],
    pub decision_lock_round: [u8; SWAPS],
    /// 0 = nonterminal, 1 = confirm, 2 = cancel.
    pub terminal: [u8; SWAPS],
    pub tombstone: [bool; SWAPS],
}

impl ValidatorState {
    const EMPTY: Self = Self {
        reservation: 0,
        highest_precommit_round: [0; SWAPS],
        decision_lock: [0; SWAPS],
        decision_lock_round: [0; SWAPS],
        terminal: [0; SWAPS],
        tombstone: [false; SWAPS],
    };
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct State {
    pub validators: [ValidatorState; MAX_VALIDATORS],
    /// Process crash/restart is modeled globally because arbitrary per-validator
    /// message scheduling already represents every partition subset. Persisted
    /// protocol facts are unchanged while `running` is false.
    pub running: bool,
    pub expired: [bool; SWAPS],
    pub fenced: bool,
    pub rotated: bool,
    pub prepare_votes: [u8; SWAPS],
    pub lock_qc: [bool; SWAPS],
    pub cancel_precommit_votes: [u8; SWAPS],
    pub cancel_precommit_qc: [bool; SWAPS],
    pub confirm_commit_votes: [u8; SWAPS],
    pub cancel_commit_votes: [u8; SWAPS],
    pub confirm_decision_qc: [bool; SWAPS],
    pub cancel_decision_qc: [bool; SWAPS],
    pub effects_votes: [u8; SWAPS],
    pub effects_qc: [bool; SWAPS],
    pub cancel_apply_votes: [u8; SWAPS],
    pub cancel_apply_qc: [bool; SWAPS],
}

impl State {
    pub fn initial(_config: ModelConfig) -> Self {
        Self {
            validators: [ValidatorState::EMPTY; MAX_VALIDATORS],
            running: true,
            expired: [false; SWAPS],
            fenced: false,
            rotated: false,
            prepare_votes: [0; SWAPS],
            lock_qc: [false; SWAPS],
            cancel_precommit_votes: [0; SWAPS],
            cancel_precommit_qc: [false; SWAPS],
            confirm_commit_votes: [0; SWAPS],
            cancel_commit_votes: [0; SWAPS],
            confirm_decision_qc: [false; SWAPS],
            cancel_decision_qc: [false; SWAPS],
            effects_votes: [0; SWAPS],
            effects_qc: [false; SWAPS],
            cancel_apply_votes: [0; SWAPS],
            cancel_apply_qc: [false; SWAPS],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct HonestFingerprint {
    local: ValidatorState,
    prepare: [bool; SWAPS],
    cancel_precommit: [bool; SWAPS],
    confirm_commit: [bool; SWAPS],
    cancel_commit: [bool; SWAPS],
    effects: [bool; SWAPS],
    cancel_apply: [bool; SWAPS],
}

fn normalize(mut state: State, config: ModelConfig) -> State {
    let n = config.validator_count as usize;
    let mut honest = (1..n)
        .map(|validator| HonestFingerprint {
            local: state.validators[validator],
            prepare: std::array::from_fn(|swap| has(state.prepare_votes[swap], validator)),
            cancel_precommit: std::array::from_fn(|swap| {
                has(state.cancel_precommit_votes[swap], validator)
            }),
            confirm_commit: std::array::from_fn(|swap| {
                has(state.confirm_commit_votes[swap], validator)
            }),
            cancel_commit: std::array::from_fn(|swap| {
                has(state.cancel_commit_votes[swap], validator)
            }),
            effects: std::array::from_fn(|swap| has(state.effects_votes[swap], validator)),
            cancel_apply: std::array::from_fn(|swap| {
                has(state.cancel_apply_votes[swap], validator)
            }),
        })
        .collect::<Vec<_>>();
    honest.sort_unstable();

    let byzantine_bit = 1_u8;
    for swap in 0..SWAPS {
        state.prepare_votes[swap] &= byzantine_bit;
        state.cancel_precommit_votes[swap] &= byzantine_bit;
        state.confirm_commit_votes[swap] &= byzantine_bit;
        state.cancel_commit_votes[swap] &= byzantine_bit;
        state.effects_votes[swap] &= byzantine_bit;
        state.cancel_apply_votes[swap] &= byzantine_bit;
    }
    for (offset, fingerprint) in honest.into_iter().enumerate() {
        let validator = offset + 1;
        state.validators[validator] = fingerprint.local;
        for swap in 0..SWAPS {
            if fingerprint.prepare[swap] {
                insert(&mut state.prepare_votes[swap], validator);
            }
            if fingerprint.cancel_precommit[swap] {
                insert(&mut state.cancel_precommit_votes[swap], validator);
            }
            if fingerprint.confirm_commit[swap] {
                insert(&mut state.confirm_commit_votes[swap], validator);
            }
            if fingerprint.cancel_commit[swap] {
                insert(&mut state.cancel_commit_votes[swap], validator);
            }
            if fingerprint.effects[swap] {
                insert(&mut state.effects_votes[swap], validator);
            }
            if fingerprint.cancel_apply[swap] {
                insert(&mut state.cancel_apply_votes[swap], validator);
            }
        }
    }
    state
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Step {
    Prepare {
        swap: usize,
        validator: usize,
    },
    FormLockQc {
        swap: usize,
    },
    Expire {
        swap: usize,
    },
    CancelPrecommit {
        swap: usize,
        validator: usize,
    },
    FormCancelPrecommitQc {
        swap: usize,
    },
    Commit {
        swap: usize,
        validator: usize,
        decision: Decision,
    },
    FormDecisionQc {
        swap: usize,
        decision: Decision,
    },
    Apply {
        swap: usize,
        validator: usize,
        decision: Decision,
    },
    FormTerminalQc {
        swap: usize,
        decision: Decision,
    },
    Crash,
    Restart,
    Fence,
    Rotate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InvariantViolation {
    ConflictingObjectConsumed { swap_a: usize, swap_b: usize },
    ConflictingDecisionCertificates { swap: usize },
    EffectsWithoutConfirm { swap: usize },
    ApplyAfterCancelTombstone { swap: usize, validator: usize },
    CancelTerminalWithoutTombstone { swap: usize, validator: usize },
    ReservationReleasedWithoutCancel { swap: usize, validator: usize },
    RotatedWithUndrainedState,
    QuorumArithmetic,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Counterexample {
    pub violation: InvariantViolation,
    pub trace: Vec<Step>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckReport {
    pub validator_count: u8,
    pub fault_tolerance: u8,
    pub quorum: u8,
    pub max_depth: u8,
    pub states_explored: usize,
    pub transitions_explored: usize,
    pub deepest_state: u8,
    pub counterexample: Option<Counterexample>,
}

fn has(mask: u8, validator: usize) -> bool {
    mask & (1_u8 << validator) != 0
}

fn insert(mask: &mut u8, validator: usize) {
    *mask |= 1_u8 << validator;
}

fn count(mask: u8) -> u8 {
    mask.count_ones() as u8
}

fn is_byzantine(validator: usize) -> bool {
    // The model places the single allowed Byzantine identity at index zero.
    validator == 0
}

fn decision_code(decision: Decision) -> u8 {
    match decision {
        Decision::Confirm => 1,
        Decision::Cancel => 2,
    }
}

fn invariant(state: &State, config: ModelConfig) -> Option<InvariantViolation> {
    if config.quorum() <= 2 * config.fault_tolerance()
        || 2 * config.quorum() <= config.validator_count + config.fault_tolerance()
    {
        return Some(InvariantViolation::QuorumArithmetic);
    }
    if state.effects_qc[0] && state.effects_qc[1] {
        return Some(InvariantViolation::ConflictingObjectConsumed {
            swap_a: 0,
            swap_b: 1,
        });
    }
    for swap in 0..SWAPS {
        if state.confirm_decision_qc[swap] && state.cancel_decision_qc[swap] {
            return Some(InvariantViolation::ConflictingDecisionCertificates { swap });
        }
        if state.effects_qc[swap] && !state.confirm_decision_qc[swap] {
            return Some(InvariantViolation::EffectsWithoutConfirm { swap });
        }
        for validator in 1..config.validator_count as usize {
            let local = state.validators[validator];
            if local.tombstone[swap] && local.terminal[swap] == 1 {
                return Some(InvariantViolation::ApplyAfterCancelTombstone { swap, validator });
            }
            if local.terminal[swap] == 2 && !local.tombstone[swap] {
                return Some(InvariantViolation::CancelTerminalWithoutTombstone {
                    swap,
                    validator,
                });
            }
            if has(state.prepare_votes[swap], validator)
                && local.reservation != (swap + 1) as u8
                && local.terminal[swap] == 0
                && !local.tombstone[swap]
            {
                return Some(InvariantViolation::ReservationReleasedWithoutCancel {
                    swap,
                    validator,
                });
            }
        }
    }
    if state.rotated {
        let undrained = state
            .validators
            .iter()
            .take(config.validator_count as usize)
            .any(|validator| validator.reservation != 0)
            || (0..SWAPS).any(|swap| {
                (state.prepare_votes[swap] != 0)
                    && !state.effects_qc[swap]
                    && !state.cancel_apply_qc[swap]
            });
        if undrained {
            return Some(InvariantViolation::RotatedWithUndrainedState);
        }
    }
    None
}

fn successors(state: State, config: ModelConfig) -> Vec<(Step, State)> {
    let n = config.validator_count as usize;
    let q = config.quorum();
    let mut out = Vec::new();

    // Rotation closes this committee-domain instance. Even a Byzantine old
    // signer cannot create a message in the new domain, and its post-rotation
    // old-domain signatures cannot contribute to a current certificate.
    if state.rotated {
        return out;
    }

    if state.running {
        let mut next = state;
        next.running = false;
        out.push((Step::Crash, next));
    } else {
        let mut next = state;
        next.running = true;
        out.push((Step::Restart, next));
        return out;
    }

    if !state.fenced {
        let mut next = state;
        next.fenced = true;
        out.push((Step::Fence, next));
    }

    for swap in 0..SWAPS {
        if !state.expired[swap] {
            let mut next = state;
            next.expired[swap] = true;
            out.push((Step::Expire { swap }, next));
        }

        for validator in 0..n {
            if has(state.prepare_votes[swap], validator) {
                continue;
            }
            let local = state.validators[validator];
            let honest_can_prepare = !state.fenced
                && !state.expired[swap]
                && local.reservation == 0
                && local.terminal[swap] == 0
                && !local.tombstone[swap];
            if honest_can_prepare || is_byzantine(validator) {
                let mut next = state;
                insert(&mut next.prepare_votes[swap], validator);
                if !is_byzantine(validator) {
                    let local = &mut next.validators[validator];
                    local.reservation = (swap + 1) as u8;
                    local.highest_precommit_round[swap] = 0;
                }
                out.push((Step::Prepare { swap, validator }, next));
            }
        }

        if !state.lock_qc[swap] && count(state.prepare_votes[swap]) >= q {
            let mut next = state;
            next.lock_qc[swap] = true;
            out.push((Step::FormLockQc { swap }, next));
        }

        for validator in 0..n {
            if has(state.cancel_precommit_votes[swap], validator) || !state.expired[swap] {
                continue;
            }
            let local = state.validators[validator];
            let honest_can_cancel = local.terminal[swap] == 0
                && local.decision_lock[swap] == 0
                && !local.tombstone[swap];
            if honest_can_cancel || is_byzantine(validator) {
                let mut next = state;
                insert(&mut next.cancel_precommit_votes[swap], validator);
                if !is_byzantine(validator) {
                    next.validators[validator].highest_precommit_round[swap] = 1;
                }
                out.push((Step::CancelPrecommit { swap, validator }, next));
            }
        }

        if !state.cancel_precommit_qc[swap] && count(state.cancel_precommit_votes[swap]) >= q {
            let mut next = state;
            next.cancel_precommit_qc[swap] = true;
            out.push((Step::FormCancelPrecommitQc { swap }, next));
        }

        for (decision, qc, round) in [
            (Decision::Confirm, state.lock_qc[swap], 0_u8),
            (Decision::Cancel, state.cancel_precommit_qc[swap], 1_u8),
        ] {
            if !qc {
                continue;
            }
            for validator in 0..n {
                let votes = match decision {
                    Decision::Confirm => state.confirm_commit_votes[swap],
                    Decision::Cancel => state.cancel_commit_votes[swap],
                };
                if has(votes, validator) {
                    continue;
                }
                let local = state.validators[validator];
                let stale_ok =
                    !config.enforce_stale_qc_guard || round >= local.highest_precommit_round[swap];
                let lock_ok = local.decision_lock[swap] == 0
                    || (local.decision_lock[swap] == decision_code(decision)
                        && round >= local.decision_lock_round[swap])
                    // Negative-control mode deliberately combines the two
                    // classic bugs: accepting a stale QC after a higher vote
                    // and then changing value merely because a later round is
                    // numerically higher, without a safe NewRound proof.
                    || (!config.enforce_stale_qc_guard
                        && round > local.decision_lock_round[swap]);
                let honest_can_commit =
                    local.terminal[swap] == 0 && !local.tombstone[swap] && stale_ok && lock_ok;
                if honest_can_commit || is_byzantine(validator) {
                    let mut next = state;
                    let target_votes = match decision {
                        Decision::Confirm => &mut next.confirm_commit_votes[swap],
                        Decision::Cancel => &mut next.cancel_commit_votes[swap],
                    };
                    insert(target_votes, validator);
                    if !is_byzantine(validator) {
                        let local = &mut next.validators[validator];
                        local.decision_lock[swap] = decision_code(decision);
                        local.decision_lock_round[swap] = round;
                    }
                    out.push((
                        Step::Commit {
                            swap,
                            validator,
                            decision,
                        },
                        next,
                    ));
                }
            }
        }

        for decision in [Decision::Confirm, Decision::Cancel] {
            let (formed, votes) = match decision {
                Decision::Confirm => (
                    state.confirm_decision_qc[swap],
                    state.confirm_commit_votes[swap],
                ),
                Decision::Cancel => (
                    state.cancel_decision_qc[swap],
                    state.cancel_commit_votes[swap],
                ),
            };
            if !formed && count(votes) >= q {
                let mut next = state;
                match decision {
                    Decision::Confirm => next.confirm_decision_qc[swap] = true,
                    Decision::Cancel => next.cancel_decision_qc[swap] = true,
                }
                out.push((Step::FormDecisionQc { swap, decision }, next));
            }
        }

        for decision in [Decision::Confirm, Decision::Cancel] {
            let decision_qc = match decision {
                Decision::Confirm => state.confirm_decision_qc[swap],
                Decision::Cancel => state.cancel_decision_qc[swap],
            };
            if !decision_qc {
                continue;
            }
            for validator in 0..n {
                let terminal_votes = match decision {
                    Decision::Confirm => state.effects_votes[swap],
                    Decision::Cancel => state.cancel_apply_votes[swap],
                };
                if has(terminal_votes, validator) {
                    continue;
                }
                let local = state.validators[validator];
                let conflict = local.reservation != 0 && local.reservation != (swap + 1) as u8;
                let honest_can_apply = local.terminal[swap] == 0
                    && !local.tombstone[swap]
                    && (!conflict || decision == Decision::Confirm);
                if honest_can_apply || is_byzantine(validator) {
                    let mut next = state;
                    match decision {
                        Decision::Confirm => insert(&mut next.effects_votes[swap], validator),
                        Decision::Cancel => insert(&mut next.cancel_apply_votes[swap], validator),
                    }
                    if !is_byzantine(validator) {
                        let local = &mut next.validators[validator];
                        local.terminal[swap] = decision_code(decision);
                        if decision == Decision::Cancel {
                            local.tombstone[swap] = true;
                        }
                        if local.reservation == (swap + 1) as u8 {
                            local.reservation = 0;
                        } else if decision == Decision::Confirm && conflict {
                            // A terminal conflicting certificate supersedes only a partial
                            // local reservation; certificate conflicts are checked above.
                            let conflicting_swap = (local.reservation - 1) as usize;
                            local.terminal[conflicting_swap] = 2;
                            local.tombstone[conflicting_swap] = true;
                            local.reservation = 0;
                        }
                    }
                    out.push((
                        Step::Apply {
                            swap,
                            validator,
                            decision,
                        },
                        next,
                    ));
                }
            }
        }

        for decision in [Decision::Confirm, Decision::Cancel] {
            let (formed, votes) = match decision {
                Decision::Confirm => (state.effects_qc[swap], state.effects_votes[swap]),
                Decision::Cancel => (state.cancel_apply_qc[swap], state.cancel_apply_votes[swap]),
            };
            if !formed && count(votes) >= q {
                let mut next = state;
                match decision {
                    Decision::Confirm => next.effects_qc[swap] = true,
                    Decision::Cancel => next.cancel_apply_qc[swap] = true,
                }
                out.push((Step::FormTerminalQc { swap, decision }, next));
            }
        }
    }

    if state.fenced && !state.rotated {
        let drained = state
            .validators
            .iter()
            .take(n)
            .all(|validator| validator.reservation == 0)
            && (0..SWAPS).all(|swap| {
                state.prepare_votes[swap] == 0
                    || state.effects_qc[swap]
                    || state.cancel_apply_qc[swap]
            });
        if drained {
            let mut next = state;
            next.rotated = true;
            out.push((Step::Rotate, next));
        }
    }

    out
}

fn trace_to(
    parents: &BTreeMap<State, (Option<State>, Option<Step>, u8)>,
    mut state: State,
) -> Vec<Step> {
    let mut trace = Vec::new();
    while let Some((Some(previous), Some(step), _)) = parents.get(&state) {
        trace.push(step.clone());
        state = *previous;
    }
    trace.reverse();
    trace
}

pub fn check(config: ModelConfig) -> Result<CheckReport, ModelError> {
    let config = config.validate()?;
    let initial = State::initial(config);
    let mut queue = VecDeque::from([initial]);
    let mut parents = BTreeMap::from([(initial, (None, None, 0_u8))]);
    let mut transitions_explored = 0_usize;
    let mut deepest_state = 0_u8;

    while let Some(state) = queue.pop_front() {
        let depth = parents.get(&state).map_or(0, |entry| entry.2);
        deepest_state = deepest_state.max(depth);
        if let Some(violation) = invariant(&state, config) {
            return Ok(CheckReport {
                validator_count: config.validator_count,
                fault_tolerance: config.fault_tolerance(),
                quorum: config.quorum(),
                max_depth: config.max_depth,
                states_explored: parents.len(),
                transitions_explored,
                deepest_state,
                counterexample: Some(Counterexample {
                    violation,
                    trace: trace_to(&parents, state),
                }),
            });
        }
        if depth >= config.max_depth {
            continue;
        }
        for (step, next) in successors(state, config) {
            transitions_explored = transitions_explored.saturating_add(1);
            let next = normalize(next, config);
            if let std::collections::btree_map::Entry::Vacant(entry) = parents.entry(next) {
                entry.insert((Some(state), Some(step), depth + 1));
                queue.push_back(next);
            }
        }
    }

    Ok(CheckReport {
        validator_count: config.validator_count,
        fault_tolerance: config.fault_tolerance(),
        quorum: config.quorum(),
        max_depth: config.max_depth,
        states_explored: parents.len(),
        transitions_explored,
        deepest_state,
        counterexample: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(n: u8, guard: bool) -> ModelConfig {
        ModelConfig {
            validator_count: n,
            max_depth: 18,
            enforce_stale_qc_guard: guard,
        }
    }

    #[test]
    fn n4_safety_model_is_counterexample_free() {
        let report = check(config(4, true)).expect("valid model configuration");
        assert_eq!(report.quorum, 3);
        assert!(report.counterexample.is_none(), "{report:#?}");
    }

    #[test]
    fn n6_safety_model_is_counterexample_free() {
        let report = check(config(6, true)).expect("valid model configuration");
        assert_eq!(report.quorum, 5);
        assert!(report.counterexample.is_none(), "{report:#?}");
    }

    #[test]
    fn stale_qc_negative_control_finds_conflicting_decisions() {
        let report = check(config(4, false)).expect("valid model configuration");
        assert!(
            matches!(
                report.counterexample,
                Some(Counterexample {
                    violation: InvariantViolation::ConflictingDecisionCertificates { .. },
                    ..
                })
            ),
            "negative control failed to find the intended safety bug: {report:#?}"
        );
    }
}
