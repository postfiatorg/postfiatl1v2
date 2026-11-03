//! `external_lane_release_v1`: native PFT leaves a genesis external lane for a
//! PFTL account against a proven Base Lock. Design:
//! `docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md` sections 5 and
//! 5.1. The lane record rules live on `ExternalLaneState`; this module finds
//! the lane, applies them and credits the recipient. Execution runs on a
//! cloned ledger, and every check here precedes the first write, so a refused
//! release changes nothing.
//!
//! The proof is design option (b) of section 4, chosen in section 10: a Base
//! block checkpoint signed by a quorum of the bridge committee the lane's
//! genesis policy names, and a receipt-inclusion proof of the lock contract's
//! `Locked` event under that checkpoint's receipts root. The checkpoint reuses
//! `EthereumFinalizedCheckpointV1`; for a lane its route fields carry the lane
//! id, the lane's route-config digest, the lock contract and the token with
//! their runtime code hashes.

use postfiat_bridge::{
    decode_locked_event, ethereum_keccak256, verify_ethereum_checkpoint_certificate,
    verify_ethereum_receipt_log,
};
use postfiat_crypto_provider::{bytes_to_hex, hex_to_bytes};
use postfiat_types::{
    EthereumExternalEventProofV1, ExternalLaneReleaseOperationV1, ExternalLaneReleaseRecord,
    FastSwapChainDomainV1, FastSwapCommitteeRootV1, FastSwapCommitteeV1, FastSwapOpaqueHashV1,
    Genesis, GenesisExternalLane, LedgerState, EXTERNAL_LANE_CHECKPOINT_MISMATCH,
    EXTERNAL_LANE_COMMITTEE_MISSING, EXTERNAL_LANE_LOCK_MISMATCH, EXTERNAL_LANE_NOT_CONFIGURED,
    EXTERNAL_LANE_PROOF_INVALID, EXTERNAL_LANE_SOURCE_MISMATCH,
};

type ExecutionError = (&'static str, String);

/// What a release claims: the Lock, keyed by source chain, lock contract and
/// nonce, with its amount and PFTL recipient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalLockClaim<'a> {
    pub lane_id: &'a str,
    pub source_chain_id: u64,
    pub lock_contract_address: &'a str,
    pub lock_nonce: u64,
    pub amount_atoms: u64,
    pub recipient: &'a str,
}

/// A Base Lock whose proof has been checked against the lane's policy.
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

fn configured_lane<'a>(
    genesis: &'a Genesis,
    lane_id: &str,
) -> Result<&'a GenesisExternalLane, ExecutionError> {
    genesis
        .external_lanes()
        .iter()
        .find(|lane| lane.id == lane_id)
        .ok_or_else(|| {
            (
                EXTERNAL_LANE_NOT_CONFIGURED,
                format!("genesis configures no external lane `{lane_id}`"),
            )
        })
}

fn require_lane_source(
    lane: &GenesisExternalLane,
    source_chain_id: u64,
    lock_contract_address: &str,
) -> Result<(), ExecutionError> {
    if source_chain_id != lane.policy.chain_id
        || lock_contract_address != lane.policy.lock_contract_address
    {
        return Err((
            EXTERNAL_LANE_SOURCE_MISMATCH,
            format!(
                "external lane `{}` releases only Locks of chain {} contract {}",
                lane.id, lane.policy.chain_id, lane.policy.lock_contract_address
            ),
        ));
    }
    Ok(())
}

fn policy_bytes<const N: usize>(
    field: &'static str,
    value: &str,
) -> Result<[u8; N], ExecutionError> {
    value
        .strip_prefix("0x")
        .and_then(|digits| hex_to_bytes(digits).ok())
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| {
            (
                EXTERNAL_LANE_CHECKPOINT_MISMATCH,
                format!("external lane policy {field} is not {N} hex bytes"),
            )
        })
}

fn genesis_chain_domain(genesis: &Genesis) -> Result<FastSwapChainDomainV1, ExecutionError> {
    let genesis_hash = hex_to_bytes(&crate::genesis_hash(genesis))
        .ok()
        .and_then(|bytes| <[u8; 48]>::try_from(bytes).ok())
        .ok_or_else(|| {
            (
                EXTERNAL_LANE_CHECKPOINT_MISMATCH,
                "genesis hash is not 48 bytes".to_string(),
            )
        })?;
    Ok(FastSwapChainDomainV1 {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: FastSwapOpaqueHashV1(genesis_hash),
        protocol_version: genesis.protocol_version,
    })
}

/// The governed committee the lane's policy names, on this chain.
fn lane_committee<'a>(
    genesis: &Genesis,
    ledger: &'a LedgerState,
    lane: &GenesisExternalLane,
) -> Result<&'a FastSwapCommitteeV1, ExecutionError> {
    let root = FastSwapCommitteeRootV1(policy_bytes::<48>(
        "committee_root",
        &lane.policy.committee_root,
    )?);
    let chain = genesis_chain_domain(genesis)?;
    ledger
        .fastswap_committees
        .iter()
        .find(|committee| {
            committee.domain.committee_epoch == lane.policy.committee_epoch
                && committee.domain.committee_root == root
                && committee.domain.chain == chain
        })
        .ok_or_else(|| {
            (
                EXTERNAL_LANE_COMMITTEE_MISSING,
                format!(
                    "no governed committee matches external lane `{}` epoch {} root {}",
                    lane.id, lane.policy.committee_epoch, lane.policy.committee_root
                ),
            )
        })
}

/// Verifies that `proof` proves exactly the claimed Lock under the lane's
/// genesis policy: a quorum checkpoint of the configured committee over a
/// block of the configured Base chain, bound to the lane, the lock contract
/// and the token with their code hashes, whose receipts include the lock
/// contract's `Locked` event with the claimed nonce, amount and recipient.
pub(crate) fn verify_external_lane_lock(
    genesis: &Genesis,
    ledger: &LedgerState,
    claim: &ExternalLockClaim<'_>,
    proof: &EthereumExternalEventProofV1,
) -> Result<VerifiedExternalLock, ExecutionError> {
    let lane = configured_lane(genesis, claim.lane_id)?;
    require_lane_source(lane, claim.source_chain_id, claim.lock_contract_address)?;
    // The attested distance from the Lock's block to the observed head, not
    // only the signed echo of the depth, must reach the policy's depth.
    let attested = &proof.checkpoint_certificate.checkpoint;
    if attested
        .observed_head_number
        .checked_sub(attested.block_number)
        .is_none_or(|distance| distance < lane.policy.confirmation_depth)
    {
        return Err((
            EXTERNAL_LANE_CHECKPOINT_MISMATCH,
            format!(
                "Base checkpoint observes head {} for block {}: fewer than the {} confirmations external lane `{}` requires",
                attested.observed_head_number,
                attested.block_number,
                lane.policy.confirmation_depth,
                lane.id
            ),
        ));
    }
    proof.validate_bounds().map_err(|error| {
        (
            EXTERNAL_LANE_PROOF_INVALID,
            format!("external lane proof is malformed: {error:?}"),
        )
    })?;
    let committee = lane_committee(genesis, ledger, lane)?;
    let (checkpoint, _) =
        verify_ethereum_checkpoint_certificate(committee, &proof.checkpoint_certificate).map_err(
            |error| {
                (
                    EXTERNAL_LANE_PROOF_INVALID,
                    format!("Base checkpoint certificate is invalid: {error:?}"),
                )
            },
        )?;
    let lock_contract =
        policy_bytes::<20>("lock_contract_address", &lane.policy.lock_contract_address)?;
    let digest = lane
        .route_config_digest()
        .map_err(|error| (EXTERNAL_LANE_CHECKPOINT_MISMATCH, error))?;
    if checkpoint.route_id != lane.id
        || checkpoint.route_config_digest != FastSwapOpaqueHashV1(digest)
        || checkpoint.ethereum_chain_id != lane.policy.chain_id
        || u64::from(checkpoint.minimum_confirmations) != lane.policy.confirmation_depth
        || checkpoint.handoff_controller != lock_contract
        || checkpoint.wrapped_navcoin_token
            != policy_bytes::<20>("token_address", &lane.policy.token_address)?
        || checkpoint.handoff_controller_code_hash
            != policy_bytes::<32>(
                "lock_runtime_code_hash",
                &lane.policy.lock_runtime_code_hash,
            )?
        || checkpoint.wrapped_navcoin_code_hash
            != policy_bytes::<32>(
                "token_runtime_code_hash",
                &lane.policy.token_runtime_code_hash,
            )?
    {
        return Err((
            EXTERNAL_LANE_CHECKPOINT_MISMATCH,
            format!(
                "Base checkpoint does not bind external lane `{}`: its chain, lane, digest, confirmations, lock contract, token and code hashes must all match the genesis policy",
                lane.id
            ),
        ));
    }
    let log_index = usize::try_from(proof.log_index).map_err(|_| {
        (
            EXTERNAL_LANE_PROOF_INVALID,
            "log index exceeds this platform's range".to_string(),
        )
    })?;
    let invalid = |error: postfiat_bridge::EthereumProofError| {
        (
            EXTERNAL_LANE_PROOF_INVALID,
            format!("Locked receipt proof is invalid: {error:?}"),
        )
    };
    let log =
        verify_ethereum_receipt_log(checkpoint.receipts_root, &proof.receipt_proof, log_index)
            .map_err(invalid)?;
    let event = decode_locked_event(&log, lock_contract).map_err(invalid)?;
    if event.nonce != claim.lock_nonce
        || event.amount_atoms != claim.amount_atoms
        || event.pftl_recipient != claim.recipient.as_bytes()
        || event.pftl_recipient_hash != ethereum_keccak256(&event.pftl_recipient)
    {
        return Err((
            EXTERNAL_LANE_LOCK_MISMATCH,
            format!(
                "Locked event (nonce {}, amount {}) does not match the claimed release of Lock nonce {} for {} atoms to `{}`",
                event.nonce, event.amount_atoms, claim.lock_nonce, claim.amount_atoms, claim.recipient
            ),
        ));
    }
    Ok(VerifiedExternalLock {
        lane_id: lane.id.clone(),
        source_chain_id: claim.source_chain_id,
        lock_contract_address: claim.lock_contract_address.to_string(),
        lock_nonce: claim.lock_nonce,
        amount_atoms: claim.amount_atoms,
        recipient: claim.recipient.to_string(),
        source_block_number: checkpoint.block_number,
        source_block_hash: format!("0x{}", bytes_to_hex(&checkpoint.block_hash)),
    })
}

/// Verifies the operation's proof against the lane's genesis policy, then
/// applies the release rules and credits the proven recipient.
pub(crate) fn execute_external_lane_release(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    operation: &ExternalLaneReleaseOperationV1,
    block_height: u64,
) -> Result<(), ExecutionError> {
    let claim = ExternalLockClaim {
        lane_id: &operation.lane_id,
        source_chain_id: operation.source_chain_id,
        lock_contract_address: &operation.lock_contract_address,
        lock_nonce: operation.lock_nonce,
        amount_atoms: operation.amount_atoms,
        recipient: &operation.recipient,
    };
    let lock = verify_external_lane_lock(genesis, ledger, &claim, &operation.proof)?;
    apply_external_lane_release(genesis, ledger, lock, block_height)
}

pub(crate) fn apply_external_lane_release(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    lock: VerifiedExternalLock,
    block_height: u64,
) -> Result<(), ExecutionError> {
    let lane = configured_lane(genesis, &lock.lane_id)?;
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
    require_lane_source(lane, lock.source_chain_id, &lock.lock_contract_address)?;
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

    pub(super) const RESERVED: u64 = 1_000_000_000;
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
                committee_root: format!("0x{}", "5e".repeat(48)),
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
        let small = ACCOUNT_RESERVE - 1;
        let (code, _) =
            apply_external_lane_release(&genesis, &mut ledger, lock(1, small, "fresh"), 1)
                .unwrap_err();
        assert_eq!(code, "below_account_reserve");
        assert_eq!(ledger, before, "the nonce stays unused");

        // The same amount to an existing funded account is released.
        ledger.ensure_account("funded").balance = ACCOUNT_RESERVE;
        ledger.accounts[0].balance -= ACCOUNT_RESERVE;
        apply_external_lane_release(&genesis, &mut ledger, lock(2, small, "funded"), 1)
            .expect("release below the reserve to a funded account");
        assert_eq!(
            ledger.account("funded").expect("funded").balance,
            ACCOUNT_RESERVE + small
        );

        // Once the fresh account exists, the refused Lock is retried and passes.
        ledger.ensure_account("fresh").balance = ACCOUNT_RESERVE;
        ledger.accounts[0].balance -= ACCOUNT_RESERVE;
        apply_external_lane_release(&genesis, &mut ledger, lock(1, small, "fresh"), 2)
            .expect("retry after the account exists");
        ledger.external_lanes[0]
            .validate_against_genesis(&genesis.external_lanes()[0])
            .expect("recorded releases");
    }
}

#[cfg(test)]
mod proof_tests {
    use super::tests::{lane_genesis, lane_ledger, LANE_ID};
    use super::*;
    use postfiat_crypto_provider::{
        ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context_seed, MlDsa65KeyPair,
    };
    use postfiat_types::{
        EthereumCheckpointCertificateV1, EthereumCheckpointVoteV1, EthereumFinalizedCheckpointV1,
        EthereumReceiptProofV1, FastSwapCommitteeDomainV1, FastSwapValidatorV1,
        ETHEREUM_CHECKPOINT_SCHEMA_V1, ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1,
        FASTSWAP_SCHEMA_VERSION_V1,
    };

    const LOCK: [u8; 20] = [0x1a; 20];
    const RECIPIENT: &str = "pfrecipient00000000000000000000000001";

    struct Fixture {
        genesis: Genesis,
        ledger: LedgerState,
        keys: Vec<MlDsa65KeyPair>,
        committee: FastSwapCommitteeV1,
    }

    fn fixture_on(chain_id: u64) -> Fixture {
        let keys: Vec<_> = (0..6_u8)
            .map(|index| ml_dsa_65_keygen_from_seed(&[0x71 + index; 32]))
            .collect();
        let mut committee = FastSwapCommitteeV1 {
            domain: FastSwapCommitteeDomainV1 {
                chain: genesis_chain_domain(&lane_genesis()).expect("domain"),
                fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
                committee_epoch: 2,
                committee_root: FastSwapCommitteeRootV1::ZERO,
                validator_count: 6,
                quorum: 5,
            },
            validators: keys
                .iter()
                .enumerate()
                .map(|(index, key)| FastSwapValidatorV1 {
                    validator_id: format!("validator-{index}"),
                    public_key: key.public_key.clone(),
                })
                .collect(),
        };
        committee.domain.committee_root = committee.computed_root().expect("root");
        let mut genesis = lane_genesis();
        let lane = &mut genesis.external_lanes.as_mut().expect("lanes")[0];
        lane.policy.chain_id = chain_id;
        lane.policy.committee_root =
            format!("0x{}", bytes_to_hex(&committee.domain.committee_root.0));
        genesis.validate().expect("lane genesis");
        committee.domain.chain = genesis_chain_domain(&genesis).expect("domain");
        let mut ledger = lane_ledger(&genesis);
        ledger.fastswap_committees.push(committee.clone());
        Fixture {
            genesis,
            ledger,
            keys,
            committee,
        }
    }

    fn rlp_bytes(bytes: &[u8]) -> Vec<u8> {
        if bytes.len() == 1 && bytes[0] <= 0x7f {
            return bytes.to_vec();
        }
        rlp_prefixed(0x80, bytes)
    }

    fn rlp_list(items: &[Vec<u8>]) -> Vec<u8> {
        rlp_prefixed(0xc0, &items.concat())
    }

    fn rlp_prefixed(base: u8, payload: &[u8]) -> Vec<u8> {
        let mut encoded = if payload.len() < 56 {
            vec![base + payload.len() as u8]
        } else {
            let length = payload.len().to_be_bytes();
            let first = length.iter().position(|byte| *byte != 0).unwrap_or(7);
            let mut prefix = vec![base + 55 + (8 - first) as u8];
            prefix.extend_from_slice(&length[first..]);
            prefix
        };
        encoded.extend_from_slice(payload);
        encoded
    }

    fn word_u64(value: u64) -> [u8; 32] {
        let mut word = [0_u8; 32];
        word[24..].copy_from_slice(&value.to_be_bytes());
        word
    }

    /// One receipt whose one log is `Locked(nonce, holder, keccak(recipient),
    /// recipient, amount)` emitted by `emitter`.
    fn locked_receipt(
        emitter: [u8; 20],
        nonce: u64,
        recipient: &str,
        amount: u64,
    ) -> ([u8; 32], EthereumReceiptProofV1) {
        let mut holder = [0_u8; 32];
        holder[12..].copy_from_slice(&[0x11; 20]);
        let topics = [
            ethereum_keccak256(b"Locked(uint64,address,bytes32,bytes,uint64)"),
            word_u64(nonce),
            holder,
            ethereum_keccak256(recipient.as_bytes()),
        ];
        let mut data = word_u64(0x40).to_vec();
        data.extend_from_slice(&word_u64(amount));
        data.extend_from_slice(&word_u64(recipient.len() as u64));
        data.extend_from_slice(recipient.as_bytes());
        data.resize(data.len().div_ceil(32) * 32, 0);
        let log = rlp_list(&[
            rlp_bytes(&emitter),
            rlp_list(
                &topics
                    .iter()
                    .map(|topic| rlp_bytes(topic))
                    .collect::<Vec<_>>(),
            ),
            rlp_bytes(&data),
        ]);
        let receipt = rlp_list(&[
            rlp_bytes(&[1]),
            rlp_bytes(&[1]),
            rlp_bytes(&[0; 256]),
            rlp_list(&[log]),
        ]);
        let leaf = rlp_list(&[rlp_bytes(&[0x20, 0x80]), rlp_bytes(&receipt)]);
        (
            ethereum_keccak256(&leaf),
            EthereumReceiptProofV1 {
                transaction_index: 0,
                receipt_rlp: receipt,
                proof_nodes_rlp: vec![leaf],
            },
        )
    }

    fn checkpoint(fx: &Fixture, receipts_root: [u8; 32]) -> EthereumFinalizedCheckpointV1 {
        let lane = &fx.genesis.external_lanes()[0];
        EthereumFinalizedCheckpointV1 {
            schema_version: ETHEREUM_CHECKPOINT_SCHEMA_V1,
            pftl_domain: fx.committee.domain.chain.clone(),
            route_id: LANE_ID.to_string(),
            route_config_digest: FastSwapOpaqueHashV1(lane.route_config_digest().expect("digest")),
            ethereum_chain_id: lane.policy.chain_id,
            block_number: 500,
            block_hash: [0x6f; 32],
            receipts_root,
            observed_head_number: 600,
            minimum_confirmations: 64,
            authority_epoch: 2,
            committee_root: fx.committee.domain.committee_root,
            handoff_controller: LOCK,
            wrapped_navcoin_token: [0x2b; 20],
            handoff_controller_code_hash: [0x4d; 32],
            wrapped_navcoin_code_hash: [0x3c; 32],
        }
    }

    fn certify(
        fx: &Fixture,
        checkpoint: EthereumFinalizedCheckpointV1,
        signers: usize,
    ) -> EthereumCheckpointCertificateV1 {
        let votes = fx
            .keys
            .iter()
            .enumerate()
            .take(signers)
            .map(|(index, key)| {
                let mut vote = EthereumCheckpointVoteV1 {
                    validator_id: format!("validator-{index}"),
                    signature: Vec::new(),
                };
                vote.signature = ml_dsa_65_sign_with_context_seed(
                    &key.private_key,
                    &vote.signing_bytes(&checkpoint).expect("vote bytes"),
                    ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1,
                    &[0xb0 + index as u8; 32],
                )
                .expect("vote signature");
                vote
            })
            .collect();
        EthereumCheckpointCertificateV1 { checkpoint, votes }
    }

    fn proof_with(
        fx: &Fixture,
        emitter: [u8; 20],
        nonce: u64,
        amount: u64,
        edit: impl FnOnce(&mut EthereumFinalizedCheckpointV1),
        signers: usize,
    ) -> EthereumExternalEventProofV1 {
        let (root, receipt_proof) = locked_receipt(emitter, nonce, RECIPIENT, amount);
        let mut checkpoint = checkpoint(fx, root);
        edit(&mut checkpoint);
        EthereumExternalEventProofV1 {
            checkpoint_certificate: certify(fx, checkpoint, signers),
            receipt_proof,
            log_index: 0,
        }
    }

    fn proof(fx: &Fixture) -> EthereumExternalEventProofV1 {
        proof_with(fx, LOCK, 7, 1_000, |_| {}, 5)
    }

    fn claim(fx: &Fixture) -> ExternalLockClaim<'_> {
        let policy = &fx.genesis.external_lanes()[0].policy;
        ExternalLockClaim {
            lane_id: LANE_ID,
            source_chain_id: policy.chain_id,
            lock_contract_address: &policy.lock_contract_address,
            lock_nonce: 7,
            amount_atoms: 1_000,
            recipient: RECIPIENT,
        }
    }

    fn refusal(
        fx: &Fixture,
        claim: &ExternalLockClaim<'_>,
        proof: &EthereumExternalEventProofV1,
    ) -> (&'static str, String) {
        verify_external_lane_lock(&fx.genesis, &fx.ledger, claim, proof).unwrap_err()
    }

    #[test]
    fn external_lane_lock_proof_of_either_base_source_verifies() {
        for chain_id in [8453, 84_532] {
            let fx = fixture_on(chain_id);
            let verified =
                verify_external_lane_lock(&fx.genesis, &fx.ledger, &claim(&fx), &proof(&fx))
                    .expect("quorum checkpoint of a Base Lock");
            assert_eq!(verified.source_chain_id, chain_id);
            assert_eq!(verified.lock_nonce, 7);
            assert_eq!(verified.source_block_number, 500);
            assert_eq!(verified.source_block_hash, format!("0x{}", "6f".repeat(32)));
            let mut ledger = fx.ledger.clone();
            apply_external_lane_release(&fx.genesis, &mut ledger, verified, 3).expect("release");
            assert_eq!(ledger.account(RECIPIENT).expect("recipient").balance, 1_000);
        }
    }

    #[test]
    fn external_lane_release_rejects_wrong_chain_contract_or_token() {
        let fx = fixture_on(8453);
        let mut foreign = claim(&fx);
        foreign.source_chain_id = 84_532;
        assert_eq!(
            refusal(&fx, &foreign, &proof(&fx)).0,
            EXTERNAL_LANE_SOURCE_MISMATCH
        );
        let edits: [fn(&mut EthereumFinalizedCheckpointV1); 7] = [
            |c| c.ethereum_chain_id = 84_532,
            |c| c.route_id = "other-lane".to_string(),
            |c| c.route_config_digest = FastSwapOpaqueHashV1([9; 48]),
            |c| c.handoff_controller = [0x99; 20],
            |c| c.wrapped_navcoin_token = [0x99; 20],
            |c| c.handoff_controller_code_hash = [0x99; 32],
            |c| c.minimum_confirmations = 63,
        ];
        for edit in edits {
            let proof = proof_with(&fx, LOCK, 7, 1_000, edit, 5);
            assert_eq!(
                refusal(&fx, &claim(&fx), &proof).0,
                EXTERNAL_LANE_CHECKPOINT_MISMATCH
            );
        }
        // A Locked event emitted by another contract under a valid checkpoint.
        let proof = proof_with(&fx, [0x99; 20], 7, 1_000, |_| {}, 5);
        let (code, message) = refusal(&fx, &claim(&fx), &proof);
        assert_eq!(code, EXTERNAL_LANE_PROOF_INVALID);
        assert!(
            message.contains("ethereum_event_emitter_mismatch"),
            "{message}"
        );
    }

    #[test]
    fn external_lane_release_requires_the_attested_confirmation_distance() {
        let fx = fixture_on(8453);
        // Head exactly `confirmation_depth` (64) past the Lock's block.
        let distant = proof_with(&fx, LOCK, 7, 1_000, |c| c.observed_head_number = 564, 5);
        verify_external_lane_lock(&fx.genesis, &fx.ledger, &claim(&fx), &distant)
            .expect("a checkpoint with the policy's distance");
        // The depth is echoed but the head is at, one short of, or below the block.
        for head in [500, 563, 499] {
            let mut at_head = proof(&fx);
            at_head
                .checkpoint_certificate
                .checkpoint
                .observed_head_number = head;
            let (code, message) = refusal(&fx, &claim(&fx), &at_head);
            assert_eq!(code, EXTERNAL_LANE_CHECKPOINT_MISMATCH, "{message}");
            assert!(message.contains("confirmations"), "{message}");
        }
    }

    #[test]
    fn external_lane_release_rejects_checkpoint_below_quorum() {
        let fx = fixture_on(8453);
        let proof = proof_with(&fx, LOCK, 7, 1_000, |_| {}, 4);
        let (code, message) = refusal(&fx, &claim(&fx), &proof);
        assert_eq!(code, EXTERNAL_LANE_PROOF_INVALID);
        assert!(message.contains("BelowQuorum"), "{message}");
    }

    #[test]
    fn external_lane_release_rejects_committee_the_policy_does_not_name() {
        let mut fx = fixture_on(8453);
        fx.ledger.fastswap_committees[0].domain.committee_epoch = 3;
        assert_eq!(
            refusal(&fx, &claim(&fx), &proof(&fx)).0,
            EXTERNAL_LANE_COMMITTEE_MISSING
        );
    }

    #[test]
    fn external_lane_release_rejects_claim_that_differs_from_the_lock() {
        let fx = fixture_on(8453);
        let proof = proof(&fx);
        let mut amount = claim(&fx);
        amount.amount_atoms = 1_001;
        let mut nonce = claim(&fx);
        nonce.lock_nonce = 8;
        let mut recipient = claim(&fx);
        recipient.recipient = "pfrecipient00000000000000000000000002";
        for claim in [amount, nonce, recipient] {
            assert_eq!(refusal(&fx, &claim, &proof).0, EXTERNAL_LANE_LOCK_MISMATCH);
        }
    }
}
