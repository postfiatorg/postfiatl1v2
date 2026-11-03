//! `external_lane_release_v1`: native PFT leaves a genesis external lane for a
//! PFTL account against a proven Base Lock. Design:
//! `docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md` sections 5 and
//! 5.1. The lane record rules live on `ExternalLaneState`; this module finds
//! the lane, applies them and credits the recipient. Execution runs on a
//! cloned ledger, and every check here precedes the first write, so a refused
//! release changes nothing.

use postfiat_types::{
    ExternalLaneReleaseRecord, Genesis, LedgerState, EXTERNAL_LANE_NOT_CONFIGURED,
    EXTERNAL_LANE_SOURCE_MISMATCH,
};

type ExecutionError = (&'static str, String);

/// A Base Lock whose proof has been checked against the lane's policy.
#[cfg_attr(not(test), allow(dead_code))] // P2.4 part 3 dispatches it
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedExternalLock {
    pub lane_id: String,
    pub source_chain_id: u64,
    pub lock_contract_address: String,
    pub lock_nonce: u64,
    pub amount_atoms: u64,
    pub recipient: String,
    pub source_block_number: u64,
    pub source_block_hash: String,
}

#[cfg_attr(not(test), allow(dead_code))] // P2.4 part 3 dispatches it
pub(crate) fn apply_external_lane_release(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    lock: VerifiedExternalLock,
    block_height: u64,
) -> Result<(), ExecutionError> {
    let lane = genesis
        .external_lanes()
        .iter()
        .find(|lane| lane.id == lock.lane_id)
        .ok_or_else(|| {
            (
                EXTERNAL_LANE_NOT_CONFIGURED,
                format!("genesis configures no external lane `{}`", lock.lane_id),
            )
        })?;
    let state_index = ledger
        .external_lanes
        .iter()
        .position(|state| state.lane_id == lane.id)
        .ok_or_else(|| {
            (
                EXTERNAL_LANE_NOT_CONFIGURED,
                format!("ledger has no record for external lane `{}`", lane.id),
            )
        })?;
    if lock.source_chain_id != lane.policy.chain_id
        || lock.lock_contract_address != lane.policy.lock_contract_address
    {
        return Err((
            EXTERNAL_LANE_SOURCE_MISMATCH,
            format!(
                "external lane `{}` releases only Locks of chain {} contract {}",
                lane.id, lane.policy.chain_id, lane.policy.lock_contract_address
            ),
        ));
    }
    ledger.external_lanes[state_index].check_release(
        lane,
        lock.lock_nonce,
        lock.amount_atoms,
        block_height,
    )?;
    let recipient_balance = ledger
        .account(&lock.recipient)
        .map_or(0, |account| account.balance)
        .checked_add(lock.amount_atoms)
        .ok_or_else(|| {
            (
                "amount_overflow",
                "external lane release overflows the recipient balance".to_string(),
            )
        })?;
    if let Some(message) = crate::account_reserve_violation(&lock.recipient, recipient_balance) {
        return Err(("below_account_reserve", message));
    }
    ledger.external_lanes[state_index].apply_release(
        lane,
        ExternalLaneReleaseRecord {
            lock_nonce: lock.lock_nonce,
            source_chain_id: lock.source_chain_id,
            lock_contract_address: lock.lock_contract_address,
            amount_atoms: lock.amount_atoms,
            recipient: lock.recipient.clone(),
            source_block_number: lock.source_block_number,
            source_block_hash: lock.source_block_hash,
            released_at_height: block_height,
        },
    )?;
    ledger.ensure_account(&lock.recipient).balance = recipient_balance;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ACCOUNT_RESERVE;
    use postfiat_types::{
        Account, GenesisExternalLane, GenesisExternalLanePolicy,
        EXTERNAL_LANE_NONCE_ALREADY_RELEASED, EXTERNAL_LANE_REMAINDER_EXCEEDED,
        EXTERNAL_LANE_WINDOW_CAP_EXCEEDED,
    };

    const RESERVED: u64 = 1_000_000_000;
    pub(super) const LANE_ID: &str = "base-erc20";

    pub(super) fn lane_genesis() -> Genesis {
        let mut genesis = Genesis::new("postfiat-lane-release");
        genesis.native_supply_atoms = Some(postfiat_types::GENESIS_NATIVE_SUPPLY_ATOMS);
        genesis.external_lanes = Some(vec![GenesisExternalLane {
            id: LANE_ID.to_string(),
            reserved_atoms: RESERVED / 2,
            release_window_blocks: 100,
            release_cap_bps: 200,
            policy: GenesisExternalLanePolicy {
                chain_id: 8453,
                lock_contract_address: format!("0x{}", "1a".repeat(20)),
                token_address: format!("0x{}", "2b".repeat(20)),
                token_runtime_code_hash: format!("0x{}", "3c".repeat(32)),
                lock_runtime_code_hash: format!("0x{}", "4d".repeat(32)),
                committee_epoch: 2,
                committee_root: format!("0x{}", "5e".repeat(32)),
                confirmation_depth: 64,
            },
        }]);
        genesis.validate().expect("lane genesis");
        genesis
    }

    pub(super) fn lane_ledger(genesis: &Genesis) -> LedgerState {
        let mut ledger = LedgerState::new(vec![Account::new(
            "faucet",
            genesis.genesis_faucet_balance_atoms().expect("faucet"),
            None,
        )]);
        ledger.external_lanes = genesis.genesis_external_lane_states();
        ledger
    }

    fn lock(nonce: u64, amount_atoms: u64, recipient: &str) -> VerifiedExternalLock {
        VerifiedExternalLock {
            lane_id: LANE_ID.to_string(),
            source_chain_id: 8453,
            lock_contract_address: format!("0x{}", "1a".repeat(20)),
            lock_nonce: nonce,
            amount_atoms,
            recipient: recipient.to_string(),
            source_block_number: 500,
            source_block_hash: format!("0x{}", "6f".repeat(32)),
        }
    }

    fn live_native(ledger: &LedgerState) -> u128 {
        let accounts: u128 = ledger.accounts.iter().map(|a| u128::from(a.balance)).sum();
        let lanes: u128 = ledger
            .external_lanes
            .iter()
            .map(|lane| u128::from(lane.remainder_atoms().expect("remainder")))
            .sum();
        accounts + lanes
    }

    #[test]
    fn external_lane_release_credits_recipient_once() {
        let genesis = lane_genesis();
        let mut ledger = lane_ledger(&genesis);
        let supply = live_native(&ledger);
        let amount = ACCOUNT_RESERVE + 1_000;
        apply_external_lane_release(&genesis, &mut ledger, lock(4, amount, "holder"), 10)
            .expect("release");
        assert_eq!(ledger.account("holder").expect("created").balance, amount);
        let lane = &ledger.external_lanes[0];
        assert_eq!(lane.released_atoms, amount);
        assert_eq!(lane.releases[0].released_at_height, 10);
        assert_eq!(
            live_native(&ledger),
            supply,
            "atoms move from the lane to the account"
        );
        lane.validate_against_genesis(&genesis.external_lanes()[0])
            .expect("recorded release");

        let before = ledger.clone();
        let (code, _) =
            apply_external_lane_release(&genesis, &mut ledger, lock(4, amount, "holder"), 11)
                .unwrap_err();
        assert_eq!(code, EXTERNAL_LANE_NONCE_ALREADY_RELEASED);
        assert_eq!(ledger, before);
    }

    #[test]
    fn external_lane_release_refuses_unconfigured_lane_and_foreign_source() {
        let genesis = lane_genesis();
        let mut ledger = lane_ledger(&genesis);
        let before = ledger.clone();
        let mut other = lock(1, ACCOUNT_RESERVE, "holder");
        other.lane_id = "other-lane".to_string();
        let (code, _) = apply_external_lane_release(&genesis, &mut ledger, other, 1).unwrap_err();
        assert_eq!(code, EXTERNAL_LANE_NOT_CONFIGURED);
        // A chain whose genesis has no lanes refuses every release.
        let laneless = Genesis::new("postfiat-laneless");
        let (code, _) = apply_external_lane_release(
            &laneless,
            &mut ledger,
            lock(1, ACCOUNT_RESERVE, "holder"),
            1,
        )
        .unwrap_err();
        assert_eq!(code, EXTERNAL_LANE_NOT_CONFIGURED);
        for edit in [
            |lock: &mut VerifiedExternalLock| lock.source_chain_id = 84_532,
            |lock: &mut VerifiedExternalLock| {
                lock.lock_contract_address = format!("0x{}", "99".repeat(20))
            },
        ] {
            let mut foreign = lock(1, ACCOUNT_RESERVE, "holder");
            edit(&mut foreign);
            let (code, _) =
                apply_external_lane_release(&genesis, &mut ledger, foreign, 1).unwrap_err();
            assert_eq!(code, EXTERNAL_LANE_SOURCE_MISMATCH);
        }
        assert_eq!(ledger, before);
    }

    #[test]
    fn external_lane_release_refuses_cap_and_remainder_without_mutation() {
        let genesis = lane_genesis();
        let cap = genesis.external_lanes()[0].window_cap_atoms();
        let mut ledger = lane_ledger(&genesis);
        let before = ledger.clone();
        let (code, _) =
            apply_external_lane_release(&genesis, &mut ledger, lock(1, cap + 1, "holder"), 5)
                .unwrap_err();
        assert_eq!(code, EXTERNAL_LANE_WINDOW_CAP_EXCEEDED);
        assert_eq!(ledger, before);
        let (code, _) =
            apply_external_lane_release(&genesis, &mut ledger, lock(1, RESERVED, "holder"), 5)
                .unwrap_err();
        assert_eq!(code, EXTERNAL_LANE_REMAINDER_EXCEEDED);
        assert_eq!(ledger, before);
        // The capped nonce is released later in a fresh window, in two parts.
        apply_external_lane_release(&genesis, &mut ledger, lock(1, cap, "holder"), 5)
            .expect("full window");
        let (code, _) =
            apply_external_lane_release(&genesis, &mut ledger, lock(2, 1, "holder"), 99)
                .unwrap_err();
        assert_eq!(code, EXTERNAL_LANE_WINDOW_CAP_EXCEEDED);
        apply_external_lane_release(&genesis, &mut ledger, lock(2, 1, "holder"), 100)
            .expect("next window");
    }

    #[test]
    fn external_lane_release_below_reserve_for_new_account_is_refused() {
        let genesis = lane_genesis();
        let mut ledger = lane_ledger(&genesis);
        let before = ledger.clone();
        let (code, _) = apply_external_lane_release(
            &genesis,
            &mut ledger,
            lock(1, ACCOUNT_RESERVE - 1, "fresh"),
            1,
        )
        .unwrap_err();
        assert_eq!(code, "below_account_reserve");
        assert_eq!(ledger, before);
    }
}
