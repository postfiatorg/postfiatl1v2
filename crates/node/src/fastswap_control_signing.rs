//! Per-validator signing and assembly of FastSwap control certificates
//! (`ActivateCommittee`, `StopPrepare`). The pattern follows
//! `ethereum-checkpoint-vote-sign`: each validator signs one vote from its own
//! key file, an anti-equivocation record is kept per (committee epoch, kind,
//! target), and a separate step assembles the certificate. Signers are the
//! active (last) FastSwap committee, as in `verify_fastlane_control_certificate`.

use super::*;
use postfiat_crypto_provider::{ml_dsa_65_sign_with_context, ml_dsa_65_verify_with_context};
use postfiat_types::{
    FastLaneControlActionV1, FastLaneControlCertificateV1, FastLaneControlVoteV1,
    FastLanePrepareFenceV1, FastSwapChainDomainV1, FastSwapCommitteeDomainV1,
    FastSwapCommitteeRootV1, FastSwapCommitteeV1, FastSwapOpaqueHashV1, FastSwapValidatorV1,
    FASTLANE_CONTROL_CONTEXT_V1, FASTSWAP_SCHEMA_VERSION_V1,
};
use zeroize::Zeroizing;

pub const FASTSWAP_CONTROL_SIGNING_STATE_DIR: &str = "fastswap-control-signing";
const FASTSWAP_CONTROL_SIGNING_STATE_SCHEMA_V1: &str = "postfiat-fastswap-control-signing-state-v1";

fn refused(message: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FastSwapControlPrepareKind {
    ActivateCommittee {
        committee_epoch: u64,
        committee_root_hex: String,
    },
    StopPrepare {
        policy_epoch: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastSwapControlPrepareOptions {
    pub data_dir: PathBuf,
    pub kind: FastSwapControlPrepareKind,
    pub control_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastSwapControlVoteSignOptions {
    pub data_dir: PathBuf,
    pub control_file: PathBuf,
    pub validator: String,
    pub validator_key_file: PathBuf,
    pub vote_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastSwapControlAssembleOptions {
    pub data_dir: PathBuf,
    pub control_file: PathBuf,
    pub vote_files: Vec<PathBuf>,
    pub certificate_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FastSwapControlSigningStateV1 {
    schema: String,
    validator_id: String,
    signer_committee_epoch: u64,
    kind: String,
    target: u64,
    action_digest: FastSwapOpaqueHashV1,
    vote: Option<FastLaneControlVoteV1>,
}

struct ControlContext {
    ledger: LedgerState,
    active: FastSwapCommitteeV1,
    tip_height: u64,
}

fn load_control_context(data_dir: &Path) -> io::Result<ControlContext> {
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let tip = read_chain_tip_or_reconstruct_for_genesis_read_only(&store, &genesis)?;
    let active = ledger
        .fastswap_committees
        .last()
        .cloned()
        .ok_or_else(|| refused("no FastSwap committee is installed"))?;
    active
        .validate()
        .map_err(|error| refused(format!("{error:?}")))?;
    if active.domain.chain != chain_domain(&genesis)? {
        return Err(refused(
            "active FastSwap committee domain does not match genesis",
        ));
    }
    Ok(ControlContext {
        ledger,
        active,
        tip_height: tip.height,
    })
}

fn chain_domain(genesis: &Genesis) -> io::Result<FastSwapChainDomainV1> {
    let hash: [u8; 48] = hex_to_bytes(&genesis_hash(genesis))
        .map_err(|error| refused(format!("genesis hash: {error:?}")))?
        .try_into()
        .map_err(|_| refused("genesis hash is not 48 bytes"))?;
    Ok(FastSwapChainDomainV1 {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: FastSwapOpaqueHashV1(hash),
        protocol_version: genesis.protocol_version,
    })
}

/// (kind, target): the anti-equivocation slot of a control action.
fn control_slot(action: &FastLaneControlActionV1) -> io::Result<(&'static str, u64)> {
    match action {
        FastLaneControlActionV1::ActivateCommittee { committee, .. } => {
            Ok(("activate_committee", committee.domain.committee_epoch))
        }
        FastLaneControlActionV1::StopPrepare { fence } => Ok(("stop_prepare", fence.policy_epoch)),
        _ => Err(refused(
            "only ActivateCommittee and StopPrepare control messages are signed here",
        )),
    }
}

/// Runs the action against a copy of the ledger exactly as
/// `execute_fastlane_control` would after the signature check.
fn dry_run_admission(context: &ControlContext, action: &FastLaneControlActionV1) -> io::Result<()> {
    let mut scratch = context.ledger.clone();
    let result = match action {
        FastLaneControlActionV1::ActivateCommittee {
            committee,
            final_checkpoint,
        } => postfiat_execution::fastswap_control::register_fastswap_committee(
            &mut scratch,
            committee.clone(),
            Some(final_checkpoint),
        ),
        FastLaneControlActionV1::StopPrepare { fence } => {
            if fence.committee_epoch != context.active.domain.committee_epoch
                || fence.finalized_primary_height == 0
                || fence.finalized_primary_height > context.tip_height
            {
                return Err(refused(
                    "StopPrepare fence must name the active committee epoch and a finalized height",
                ));
            }
            postfiat_execution::fastswap_control::apply_fastlane_prepare_fence(
                &mut scratch,
                fence.clone(),
            )
        }
        _ => return control_slot(action).map(|_| ()),
    };
    result.map_err(|error| refused(format!("control message is not admissible: {error:?}")))
}

pub fn fastswap_control_prepare(
    options: FastSwapControlPrepareOptions,
) -> io::Result<serde_json::Value> {
    let context = load_control_context(&options.data_dir)?;
    let active_epoch = context.active.domain.committee_epoch;
    let action = match &options.kind {
        FastSwapControlPrepareKind::ActivateCommittee {
            committee_epoch,
            committee_root_hex,
        } => {
            if *committee_epoch != active_epoch.saturating_add(1) {
                return Err(refused(format!(
                    "committee epoch must be {} (active epoch {active_epoch} + 1)",
                    active_epoch.saturating_add(1)
                )));
            }
            let committee =
                registry_committee(&options.data_dir, &context.active, *committee_epoch)?;
            let root_hex = bytes_to_hex(&committee.domain.committee_root.0);
            if !root_hex.eq_ignore_ascii_case(committee_root_hex.trim_start_matches("0x")) {
                return Err(refused(format!(
                    "--committee-root does not match the registry committee root {root_hex}"
                )));
            }
            let final_checkpoint = context
                .ledger
                .fast_lane_checkpoint_anchors
                .iter()
                .rev()
                .find(|certificate| {
                    certificate.votes.first().is_some_and(|vote| {
                        vote.checkpoint.committee == context.active.domain
                            && vote.checkpoint.drain_ready
                    })
                })
                .cloned()
                .ok_or_else(|| {
                    refused(format!(
                        "no drained final checkpoint of committee epoch {active_epoch} is anchored"
                    ))
                })?;
            FastLaneControlActionV1::ActivateCommittee {
                committee,
                final_checkpoint,
            }
        }
        FastSwapControlPrepareKind::StopPrepare { policy_epoch } => {
            FastLaneControlActionV1::StopPrepare {
                fence: FastLanePrepareFenceV1 {
                    committee_epoch: active_epoch,
                    policy_epoch: *policy_epoch,
                    finalized_primary_height: context.tip_height,
                },
            }
        }
    };
    dry_run_admission(&context, &action)?;
    write_json_file(&options.control_file, &action)?;
    let digest = action
        .digest()
        .map_err(|error| refused(format!("{error:?}")))?;
    let mut summary = serde_json::json!({
        "schema": "postfiat-fastswap-control-prepare-v1",
        "kind": control_slot(&action)?.0,
        "action_digest": bytes_to_hex(&digest.0),
        "signer_committee_epoch": active_epoch,
        "signer_committee_root": bytes_to_hex(&context.active.domain.committee_root.0),
        "signatures_required": context.active.domain.quorum,
        "control_file": options.control_file.display().to_string(),
    });
    if let FastLaneControlActionV1::ActivateCommittee {
        committee,
        final_checkpoint,
    } = &action
    {
        let checkpoint_id = final_checkpoint.votes[0]
            .checkpoint
            .checkpoint_id()
            .map_err(|error| refused(format!("{error:?}")))?;
        summary["committee_epoch"] = committee.domain.committee_epoch.into();
        summary["committee_root"] = bytes_to_hex(&committee.domain.committee_root.0).into();
        summary["validators"] = committee
            .validators
            .iter()
            .map(|validator| validator.validator_id.clone())
            .collect::<Vec<_>>()
            .into();
        summary["final_checkpoint_id"] = bytes_to_hex(&checkpoint_id.0).into();
    }
    Ok(summary)
}

/// The next committee over the node's current active registry keys.
fn registry_committee(
    data_dir: &Path,
    active: &FastSwapCommitteeV1,
    committee_epoch: u64,
) -> io::Result<FastSwapCommitteeV1> {
    let governance = NodeStore::new(data_dir).read_governance()?;
    let active_ids = active_validator_ids(&governance)?;
    let mut validators = load_validator_pubkeys(data_dir)?
        .into_iter()
        .filter(|(id, _)| active_ids.contains(id))
        .map(|(validator_id, public_key_hex)| {
            Ok(FastSwapValidatorV1 {
                validator_id,
                public_key: hex_to_bytes(&public_key_hex)
                    .map_err(|error| refused(format!("registry key: {error:?}")))?,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    validators.sort_by(|left, right| left.validator_id.cmp(&right.validator_id));
    let count = u16::try_from(validators.len()).map_err(|_| refused("too many validators"))?;
    let mut committee = FastSwapCommitteeV1 {
        domain: FastSwapCommitteeDomainV1 {
            chain: active.domain.chain.clone(),
            fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
            committee_epoch,
            committee_root: FastSwapCommitteeRootV1::ZERO,
            validator_count: count,
            quorum: (2 * count) / 3 + 1,
        },
        validators,
    };
    committee.domain.committee_root = committee
        .computed_root()
        .map_err(|error| refused(format!("{error:?}")))?;
    committee
        .validate()
        .map_err(|error| refused(format!("registry committee: {error:?}")))?;
    Ok(committee)
}

pub fn fastswap_control_vote_sign(
    options: FastSwapControlVoteSignOptions,
) -> io::Result<FastLaneControlVoteV1> {
    let action: FastLaneControlActionV1 =
        read_json_file(&options.control_file, "FastSwap control message")?;
    let (kind, target) = control_slot(&action)?;
    let context = load_control_context(&options.data_dir)?;
    dry_run_admission(&context, &action)?;
    let member = context
        .active
        .validators
        .iter()
        .find(|validator| validator.validator_id == options.validator)
        .ok_or_else(|| refused("signer is not in the active FastSwap committee"))?;
    let key_file = read_validator_key_file(&options.validator_key_file)?;
    let key_record = validator_key_record(&key_file, &options.validator)?;
    if key_record.algorithm_id != ML_DSA_65_ALGORITHM
        || hex_to_bytes(&key_record.public_key_hex)
            .map_err(|error| refused(format!("{error:?}")))?
            != member.public_key
    {
        return Err(refused(
            "signing key does not match the active FastSwap committee",
        ));
    }

    let action_digest = action
        .digest()
        .map_err(|error| refused(format!("{error:?}")))?;
    let domain = &context.active.domain;
    let state_path = options
        .data_dir
        .join(FASTSWAP_CONTROL_SIGNING_STATE_DIR)
        .join(format!(
            "{}-epoch{}-{kind}-{target}.json",
            options.validator, domain.committee_epoch
        ));
    let intended = FastSwapControlSigningStateV1 {
        schema: FASTSWAP_CONTROL_SIGNING_STATE_SCHEMA_V1.to_string(),
        validator_id: options.validator.clone(),
        signer_committee_epoch: domain.committee_epoch,
        kind: kind.to_string(),
        target,
        action_digest,
        vote: None,
    };
    let mut contents = serde_json::to_vec_pretty(&intended).map_err(io::Error::other)?;
    contents.push(b'\n');
    let mut state =
        match crate::ethereum_checkpoint_signing::create_file_once(&state_path, &contents) {
            Ok(()) => intended,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                read_json_file(&state_path, "FastSwap control signing state")?
            }
            Err(error) => return Err(error),
        };
    if state.schema != FASTSWAP_CONTROL_SIGNING_STATE_SCHEMA_V1
        || state.validator_id != options.validator
        || state.signer_committee_epoch != domain.committee_epoch
        || state.kind != kind
        || state.target != target
    {
        return Err(refused(
            "FastSwap control signing state has the wrong domain",
        ));
    }
    if state.action_digest != action_digest {
        return Err(refused(format!(
            "validator already signed a different {kind} for target {target} under committee epoch {}",
            domain.committee_epoch
        )));
    }
    if let Some(vote) = &state.vote {
        verify_control_vote(&context.active, &action_digest, vote)?;
        write_json_file(&options.vote_file, vote)?;
        return Ok(vote.clone());
    }

    let mut vote = FastLaneControlVoteV1 {
        committee: domain.clone(),
        action_digest,
        validator_id: options.validator.clone(),
        signature: Vec::new(),
    };
    let signing_bytes = vote
        .signing_bytes()
        .map_err(|error| refused(format!("{error:?}")))?;
    let private_key = Zeroizing::new(
        hex_to_bytes(&key_record.private_key_hex)
            .map_err(|_| refused("validator key is not hex"))?,
    );
    vote.signature =
        ml_dsa_65_sign_with_context(&private_key, &signing_bytes, FASTLANE_CONTROL_CONTEXT_V1)
            .map_err(|_| refused("ML-DSA-65 signing failed"))?;
    state.vote = Some(vote.clone());
    write_json_file(&state_path, &state)?;
    set_private_file_permissions(&state_path)?;
    write_json_file(&options.vote_file, &vote)?;
    Ok(vote)
}

fn verify_control_vote(
    committee: &FastSwapCommitteeV1,
    action_digest: &FastSwapOpaqueHashV1,
    vote: &FastLaneControlVoteV1,
) -> io::Result<()> {
    let member = committee
        .validators
        .iter()
        .find(|validator| validator.validator_id == vote.validator_id)
        .ok_or_else(|| {
            refused(format!(
                "vote from {} is not from the active FastSwap committee",
                vote.validator_id
            ))
        })?;
    let bytes = vote
        .signing_bytes()
        .map_err(|error| refused(format!("{error:?}")))?;
    if vote.committee != committee.domain
        || &vote.action_digest != action_digest
        || !ml_dsa_65_verify_with_context(
            &member.public_key,
            &bytes,
            &vote.signature,
            FASTLANE_CONTROL_CONTEXT_V1,
        )
    {
        return Err(refused(format!(
            "vote from {} does not verify against the active FastSwap committee and this control message",
            vote.validator_id
        )));
    }
    Ok(())
}

pub fn fastswap_control_assemble(
    options: FastSwapControlAssembleOptions,
) -> io::Result<FastLaneControlCertificateV1> {
    let action: FastLaneControlActionV1 =
        read_json_file(&options.control_file, "FastSwap control message")?;
    control_slot(&action)?;
    let context = load_control_context(&options.data_dir)?;
    let action_digest = action
        .digest()
        .map_err(|error| refused(format!("{error:?}")))?;
    let mut votes = options
        .vote_files
        .iter()
        .map(|path| read_json_file(path, "FastSwap control vote"))
        .collect::<io::Result<Vec<FastLaneControlVoteV1>>>()?;
    votes.sort_by(|left, right| left.validator_id.cmp(&right.validator_id));
    if votes
        .windows(2)
        .any(|pair| pair[0].validator_id == pair[1].validator_id)
    {
        return Err(refused("duplicate vote from one validator"));
    }
    for vote in &votes {
        verify_control_vote(&context.active, &action_digest, vote)?;
    }
    let quorum = usize::from(context.active.domain.quorum);
    if votes.len() < quorum {
        return Err(refused(format!(
            "{} valid votes, quorum is {quorum}",
            votes.len()
        )));
    }
    let certificate = FastLaneControlCertificateV1 { action, votes };
    let mut scratch = context.ledger.clone();
    postfiat_execution::fastswap_control::execute_fastlane_control(
        &mut scratch,
        &certificate,
        context.tip_height,
    )
    .map_err(|error| refused(format!("certificate is not admissible: {error:?}")))?;
    write_json_file(&options.certificate_file, &certificate)?;
    Ok(certificate)
}

fn write_json_file(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let json = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    atomic_write(path, format!("{json}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_crypto_provider::ml_dsa_65_keygen_from_seed;
    use postfiat_execution::fastswap_checkpoint::{
        anchor_fastlane_checkpoint, build_fastlane_checkpoint,
    };
    use postfiat_execution::fastswap_control::execute_fastlane_control;
    use postfiat_types::{
        FastLaneCheckpointCertificateV1, FastLaneCheckpointVoteV1, FastLaneStateV1,
        FASTLANE_CHECKPOINT_CONTEXT_V1,
    };

    /// Six validators, quorum 5. Epoch 1 holds validator-5's pre-rotation key;
    /// the registry (and validator-5's key file) holds its rotated key.
    struct Fixture {
        root: PathBuf,
        epoch1: FastSwapCommitteeV1,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn fixture(name: &str) -> Fixture {
        let root = std::env::temp_dir().join(format!(
            "postfiat-fastswap-control-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        init(InitOptions {
            data_dir: root.clone(),
            chain_id: "postfiat-fastswap-control-test".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 6,
        })
        .unwrap();
        let store = NodeStore::new(&root);
        let genesis = store.read_genesis().unwrap();
        let pre_rotation_v5 = ml_dsa_65_keygen_from_seed(&[99; 32]).public_key;
        let validators = load_validator_pubkeys(&root)
            .unwrap()
            .into_iter()
            .map(|(validator_id, hex)| FastSwapValidatorV1 {
                public_key: if validator_id == "validator-5" {
                    pre_rotation_v5.clone()
                } else {
                    hex_to_bytes(&hex).unwrap()
                },
                validator_id,
            })
            .collect::<Vec<_>>();
        let mut epoch1 = FastSwapCommitteeV1 {
            domain: FastSwapCommitteeDomainV1 {
                chain: chain_domain(&genesis).unwrap(),
                fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
                committee_epoch: 1,
                committee_root: FastSwapCommitteeRootV1::ZERO,
                validator_count: 6,
                quorum: 5,
            },
            validators,
        };
        epoch1.domain.committee_root = epoch1.computed_root().unwrap();
        let mut ledger = store.read_ledger().unwrap();
        ledger.fastswap_committees.push(epoch1.clone());
        store.write_ledger(&ledger).unwrap();
        Fixture { root, epoch1 }
    }

    fn key_file(fx: &Fixture) -> PathBuf {
        fx.root.join(VALIDATOR_KEYS_FILE)
    }

    fn private_key(fx: &Fixture, validator: &str) -> Vec<u8> {
        let keys = read_validator_key_file(&key_file(fx)).unwrap();
        hex_to_bytes(
            &validator_key_record(&keys, validator)
                .unwrap()
                .private_key_hex,
        )
        .unwrap()
    }

    /// Validators 0-4 sign and anchor a drained final epoch-1 checkpoint.
    fn anchor_drained_checkpoint(fx: &Fixture) {
        let store = NodeStore::new(&fx.root);
        let mut ledger = store.read_ledger().unwrap();
        let certificate = drained_checkpoint_certificate(fx);
        anchor_fastlane_checkpoint(&mut ledger, &certificate).unwrap();
        store.write_ledger(&ledger).unwrap();
    }

    /// Five-vote certificate over the drained final epoch-1 checkpoint.
    fn drained_checkpoint_certificate(fx: &Fixture) -> FastLaneCheckpointCertificateV1 {
        let ledger = NodeStore::new(&fx.root).read_ledger().unwrap();
        let state = FastLaneStateV1::empty(fx.epoch1.domain.clone());
        let checkpoint = build_fastlane_checkpoint(&state, &ledger, None, 1).unwrap();
        assert!(checkpoint.drain_ready);
        sign_checkpoint(fx, &checkpoint)
    }

    fn sign_checkpoint(
        fx: &Fixture,
        checkpoint: &postfiat_types::FastLaneCheckpointV1,
    ) -> FastLaneCheckpointCertificateV1 {
        let votes = (0..5)
            .map(|index| {
                let mut vote = FastLaneCheckpointVoteV1 {
                    checkpoint: checkpoint.clone(),
                    validator_id: format!("validator-{index}"),
                    signature: Vec::new(),
                };
                vote.signature = ml_dsa_65_sign_with_context(
                    &private_key(fx, &vote.validator_id),
                    &vote.signing_bytes().unwrap(),
                    FASTLANE_CHECKPOINT_CONTEXT_V1,
                )
                .unwrap();
                vote
            })
            .collect();
        FastLaneCheckpointCertificateV1 { votes }
    }

    include!("solana_exit_release_height_fastlane_cases.rs");

    /// `release_height_fastlane_cases::outcomes()` run on origin/main a720cc80f.
    const RELEASE_HEIGHT_FASTLANE_GOLDEN: [(u64, &str, &str); 3] = [
        (1840, "true: fastlane_checkpoint_anchored: FastLane checkpoint anchored", "revision-8 reserve None"),
        (1841, "false: solana_exit_unmatched_supply_change: FastLane checkpoint fees cannot burn revision-8 reserves", "ledger unchanged"),
        (1842, "false: solana_exit_unmatched_supply_change: FastLane checkpoint fees cannot burn revision-8 reserves", "ledger unchanged"),
    ];

    #[test]
    fn unset_second_release_refuses_fastlane_fee_burns_as_the_first_release() {
        postfiat_types::solana_exit::set_second_release_for_tests(None);
        let golden: Vec<(u64, String, String)> = RELEASE_HEIGHT_FASTLANE_GOLDEN
            .iter()
            .map(|(h, v, d)| (*h, v.to_string(), d.to_string()))
            .collect();
        assert_eq!(release_height_fastlane_cases::outcomes(), golden);
    }

    #[test]
    #[ignore = "release rehearsal at height 1760"]
    fn exit_review5_fastlane_checkpoint_cannot_burn_revision8_reserves() {
        let fx = fixture("revision8-fee");
        let mut genesis = Genesis::new_with_validator_count("postfiat-wan-devnet-2", 6);
        genesis.consensus_v2_activation_height = Some(1);
        let asset_id = postfiat_types::FastAssetIdV1(
            hex_to_bytes(postfiat_types::solana_exit::SERIES)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        let mut ledger = NodeStore::new(&fx.root).read_ledger().unwrap();
        let mut committee = fx.epoch1.clone();
        committee.domain.chain = chain_domain(&genesis).unwrap();
        committee.domain.committee_root = committee.computed_root().unwrap();
        ledger.fastswap_committees = vec![committee.clone()];
        ledger
            .fast_lane_reserves
            .push(postfiat_types::FastLaneReserveBalanceV1 {
                asset_id,
                amount_atoms: 7,
            });
        let mut state = FastLaneStateV1::empty(committee.domain.clone());
        state.pending_fee_burns.insert(asset_id, 7);
        let checkpoint = build_fastlane_checkpoint(&state, &ledger, None, 1).unwrap();
        let tx = postfiat_types::FastLanePrimaryTransactionV1 {
            operation: postfiat_types::FastLanePrimaryOperationV1::AnchorCheckpoint {
                certificate: sign_checkpoint(&fx, &checkpoint),
            },
        };
        let run = |ledger: &mut LedgerState, h| {
            crate::execution_actions::execute_fastlane_primary_for_chain(&genesis, ledger, &tx, h)
        };
        assert!(run(&mut ledger.clone(), 1759).accepted);
        assert_eq!(
            run(&mut ledger, 1760).code,
            "solana_exit_unmatched_supply_change"
        );
        assert_eq!(ledger.fast_lane_reserves[0].amount_atoms, 7);
    }

    /// Executes `operation` as block execution does and asserts the RPC
    /// finality handler reports its receipt as success.
    fn execute_reported_success(
        fx: &Fixture,
        operation: postfiat_types::FastLanePrimaryOperationV1,
        height: u64,
        expected_code: &str,
    ) {
        let store = NodeStore::new(&fx.root);
        let mut ledger = store.read_ledger().unwrap();
        let transaction = postfiat_types::FastLanePrimaryTransactionV1 { operation };
        let receipt = crate::execution_actions::execute_fastlane_primary_for_chain(
            &store.read_genesis().unwrap(),
            &mut ledger,
            &transaction,
            height,
        );
        assert!(receipt.accepted, "{receipt:?}");
        assert_eq!(receipt.code, expected_code);
        assert!(
            crate::fastlane_primary_receipt_is_success(&transaction.operation, &receipt),
            "finality submit must report {expected_code} as success"
        );
        store.write_ledger(&ledger).unwrap();
    }

    #[test]
    fn rpc_finality_reports_five_vote_anchor_then_activate_committee_as_success() {
        let fx = fixture("rpc-finality-rotation");
        let certificate = drained_checkpoint_certificate(&fx);
        assert_eq!(certificate.votes.len(), 5);
        execute_reported_success(
            &fx,
            postfiat_types::FastLanePrimaryOperationV1::AnchorCheckpoint { certificate },
            1,
            postfiat_execution::fastlane_primary::FASTLANE_CHECKPOINT_ANCHORED,
        );
        let (epoch2, root) = registry_root(&fx);
        let control = prepare_activation(&fx, 2, &root).unwrap();
        let votes = (0..5)
            .map(|index| sign(&fx, &control, &format!("validator-{index}")).unwrap())
            .collect::<Vec<_>>();
        let certificate = assemble(&fx, &control, votes).unwrap();
        execute_reported_success(
            &fx,
            postfiat_types::FastLanePrimaryOperationV1::Control { certificate },
            1,
            postfiat_execution::fastlane_primary::FASTLANE_CONTROL_APPLIED,
        );
        let ledger = NodeStore::new(&fx.root).read_ledger().unwrap();
        assert_eq!(ledger.fastswap_committees.last(), Some(&epoch2));
        // A rejected receipt is never reported as success.
        let rejected = postfiat_types::Receipt::rejected("x", "fastlane_control_rejected", "no");
        assert!(!crate::fastlane_primary_receipt_is_success(
            &postfiat_types::FastLanePrimaryOperationV1::AnchorCheckpoint {
                certificate: FastLaneCheckpointCertificateV1 { votes: Vec::new() },
            },
            &rejected
        ));
    }

    #[test]
    fn rpc_finality_reports_stop_prepare_as_success() {
        let fx = fixture("rpc-finality-stop-prepare");
        add_policy_and_tip(&fx, 7);
        let control = prepare_stop(&fx, 1).unwrap();
        let votes = (0..5)
            .map(|index| sign(&fx, &control, &format!("validator-{index}")).unwrap())
            .collect::<Vec<_>>();
        let certificate = assemble(&fx, &control, votes).unwrap();
        execute_reported_success(
            &fx,
            postfiat_types::FastLanePrimaryOperationV1::Control { certificate },
            7,
            postfiat_execution::fastlane_primary::FASTLANE_CONTROL_APPLIED,
        );
    }

    fn prepare_activation(fx: &Fixture, epoch: u64, root_hex: &str) -> io::Result<PathBuf> {
        let control_file = fx.root.join(format!("activate-{epoch}.json"));
        fastswap_control_prepare(FastSwapControlPrepareOptions {
            data_dir: fx.root.clone(),
            kind: FastSwapControlPrepareKind::ActivateCommittee {
                committee_epoch: epoch,
                committee_root_hex: root_hex.to_string(),
            },
            control_file: control_file.clone(),
        })?;
        Ok(control_file)
    }

    fn sign(fx: &Fixture, control_file: &Path, validator: &str) -> io::Result<PathBuf> {
        let vote_file = fx.root.join(format!("{validator}.control-vote.json"));
        fastswap_control_vote_sign(FastSwapControlVoteSignOptions {
            data_dir: fx.root.clone(),
            control_file: control_file.to_path_buf(),
            validator: validator.to_string(),
            validator_key_file: key_file(fx),
            vote_file: vote_file.clone(),
        })?;
        Ok(vote_file)
    }

    fn assemble(
        fx: &Fixture,
        control_file: &Path,
        vote_files: Vec<PathBuf>,
    ) -> io::Result<FastLaneControlCertificateV1> {
        fastswap_control_assemble(FastSwapControlAssembleOptions {
            data_dir: fx.root.clone(),
            control_file: control_file.to_path_buf(),
            vote_files,
            certificate_file: fx.root.join("control-certificate.json"),
        })
    }

    fn registry_root(fx: &Fixture) -> (FastSwapCommitteeV1, String) {
        let committee = registry_committee(&fx.root, &fx.epoch1, 2).unwrap();
        let root = bytes_to_hex(&committee.domain.committee_root.0);
        (committee, root)
    }

    #[test]
    fn fastswap_control_prepare_refuses_without_drained_final_checkpoint() {
        let fx = fixture("no-final");
        let (_, root) = registry_root(&fx);
        let error = prepare_activation(&fx, 2, &root).unwrap_err().to_string();
        assert!(
            error.contains("no drained final checkpoint of committee epoch 1"),
            "{error}"
        );
        anchor_drained_checkpoint(&fx);
        let error = prepare_activation(&fx, 3, &root).unwrap_err().to_string();
        assert!(error.contains("committee epoch must be 2"), "{error}");
        let error = prepare_activation(&fx, 2, &"00".repeat(48))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("does not match the registry committee root"),
            "{error}"
        );
        let error = fastswap_control_prepare(FastSwapControlPrepareOptions {
            data_dir: fx.root.clone(),
            kind: FastSwapControlPrepareKind::StopPrepare { policy_epoch: 1 },
            control_file: fx.root.join("fence.json"),
        })
        .unwrap_err()
        .to_string();
        // The fixture tip is height 0, so no finalized height can be fenced yet.
        assert!(error.contains("StopPrepare fence must name"), "{error}");
        prepare_activation(&fx, 2, &root).expect("prepare after the drained anchor");
    }

    #[test]
    fn fastswap_control_assembled_activation_is_admitted_by_six_validator_fixture() {
        let fx = fixture("admit");
        anchor_drained_checkpoint(&fx);
        let (epoch2, root) = registry_root(&fx);
        let control = prepare_activation(&fx, 2, &root).unwrap();
        let votes = (0..5)
            .map(|index| sign(&fx, &control, &format!("validator-{index}")).unwrap())
            .collect::<Vec<_>>();
        // Validator-5's key file holds its rotated key, not its epoch-1 key.
        let error = sign(&fx, &control, "validator-5").unwrap_err().to_string();
        assert!(
            error.contains("does not match the active FastSwap committee"),
            "{error}"
        );
        // Re-signing the same message returns the recorded vote.
        let first: FastLaneControlVoteV1 = read_json_file(&votes[0], "vote").unwrap();
        sign(&fx, &control, "validator-0").unwrap();
        assert_eq!(first, read_json_file(&votes[0], "vote").unwrap());

        let certificate = assemble(&fx, &control, votes).unwrap();
        let written: FastLaneControlCertificateV1 =
            read_json_file(&fx.root.join("control-certificate.json"), "certificate").unwrap();
        assert_eq!(written, certificate);
        let mut ledger = NodeStore::new(&fx.root).read_ledger().unwrap();
        execute_fastlane_control(&mut ledger, &written, 1).expect("admission accepts");
        assert_eq!(ledger.fastswap_committees.last(), Some(&epoch2));
    }

    #[test]
    fn fastswap_control_assemble_refuses_rotated_validator_5_vote_and_four_votes() {
        let fx = fixture("refuse");
        anchor_drained_checkpoint(&fx);
        let (_, root) = registry_root(&fx);
        let control = prepare_activation(&fx, 2, &root).unwrap();
        let mut votes = (0..4)
            .map(|index| sign(&fx, &control, &format!("validator-{index}")).unwrap())
            .collect::<Vec<_>>();
        let error = assemble(&fx, &control, votes.clone())
            .unwrap_err()
            .to_string();
        assert!(error.contains("4 valid votes, quorum is 5"), "{error}");

        // A vote signed by validator-5's current key bypassing vote-sign.
        let action: FastLaneControlActionV1 = read_json_file(&control, "control").unwrap();
        let mut vote = FastLaneControlVoteV1 {
            committee: fx.epoch1.domain.clone(),
            action_digest: action.digest().unwrap(),
            validator_id: "validator-5".to_string(),
            signature: Vec::new(),
        };
        vote.signature = ml_dsa_65_sign_with_context(
            &private_key(&fx, "validator-5"),
            &vote.signing_bytes().unwrap(),
            FASTLANE_CONTROL_CONTEXT_V1,
        )
        .unwrap();
        let v5_file = fx.root.join("validator-5.rotated-vote.json");
        write_json_file(&v5_file, &vote).unwrap();
        votes.push(v5_file);
        let error = assemble(&fx, &control, votes).unwrap_err().to_string();
        assert!(
            error.contains("vote from validator-5 does not verify"),
            "{error}"
        );
        assert!(!fx.root.join("control-certificate.json").exists());
    }

    /// One epoch-1 FastSwap policy (the pair and rule values of the
    /// `fastswap_service` fixture) and a finalized tip, so a fence can name it.
    fn add_policy_and_tip(fx: &Fixture, tip_height: u64) {
        use postfiat_types::{
            FastAssetIdV1, FastAssetRuleHashV1, FastSwapMarketEnvelopeHashV1, FastSwapPolicyHashV1,
            FastSwapPolicySnapshotV1, FastSwapQuoteRoundingV1,
        };
        let mut policy = FastSwapPolicySnapshotV1 {
            domain: fx.epoch1.domain.chain.clone(),
            policy_epoch: 1,
            policy_hash: FastSwapPolicyHashV1::ZERO,
            pair_asset_0: FastAssetIdV1([1; 48]),
            pair_asset_1: FastAssetIdV1([2; 48]),
            asset_rule_hash_0: FastAssetRuleHashV1([3; 48]),
            asset_rule_hash_1: FastAssetRuleHashV1([4; 48]),
            price_numerator: 1,
            price_denominator: 8,
            rounding: FastSwapQuoteRoundingV1::Exact,
            nav_epoch: 59,
            market_envelope_hash: FastSwapMarketEnvelopeHashV1([6; 48]),
            valid_from_height: 1,
            valid_through_height: 10_000,
            fee_schedule_hash: FastSwapOpaqueHashV1([10; 48]),
            max_inputs_per_party: 16,
            max_outputs: 8,
            paused: false,
        };
        policy.policy_hash = policy.computed_hash().unwrap();
        policy.validate().unwrap();
        let store = NodeStore::new(&fx.root);
        let mut ledger = store.read_ledger().unwrap();
        ledger.fastswap_policy_snapshots.push(policy);
        store.write_ledger(&ledger).unwrap();
        let genesis = store.read_genesis().unwrap();
        store
            .write_chain_tip(&ChainTipState {
                schema: CHAIN_TIP_SCHEMA.to_string(),
                chain_id: genesis.chain_id.clone(),
                genesis_hash: genesis_hash(&genesis),
                protocol_version: genesis.protocol_version,
                height: tip_height,
                block_hash: "aa".repeat(48),
                state_root: "bb".repeat(48),
                ordered_batch_count: 0,
                receipt_count: 0,
                history_base_height: 0,
            })
            .unwrap();
    }

    fn prepare_stop(fx: &Fixture, policy_epoch: u64) -> io::Result<PathBuf> {
        let control_file = fx.root.join(format!("stop-prepare-{policy_epoch}.json"));
        fastswap_control_prepare(FastSwapControlPrepareOptions {
            data_dir: fx.root.clone(),
            kind: FastSwapControlPrepareKind::StopPrepare { policy_epoch },
            control_file: control_file.clone(),
        })?;
        Ok(control_file)
    }

    #[test]
    fn fastswap_control_stop_prepare_certificate_is_admitted_and_fences_the_policy() {
        let fx = fixture("stop-prepare");
        add_policy_and_tip(&fx, 7);
        let error = prepare_stop(&fx, 2).unwrap_err().to_string();
        assert!(error.contains("InvalidFence"), "{error}");
        let control = prepare_stop(&fx, 1).unwrap();
        let expected_fence = FastLanePrepareFenceV1 {
            committee_epoch: 1,
            policy_epoch: 1,
            finalized_primary_height: 7,
        };
        assert_eq!(
            read_json_file::<FastLaneControlActionV1>(&control, "control").unwrap(),
            FastLaneControlActionV1::StopPrepare {
                fence: expected_fence.clone()
            }
        );
        let votes = (0..5)
            .map(|index| sign(&fx, &control, &format!("validator-{index}")).unwrap())
            .collect::<Vec<_>>();
        let certificate = assemble(&fx, &control, votes).unwrap();
        assert_eq!(certificate.votes.len(), 5);

        // Mempool admission and the block execution path both accept it.
        let transaction = postfiat_types::FastLanePrimaryTransactionV1 {
            operation: postfiat_types::FastLanePrimaryOperationV1::Control {
                certificate: certificate.clone(),
            },
        };
        admit_fastlane_primary_to_mempool(&fx.root, transaction.clone())
            .expect("mempool admission");
        let store = NodeStore::new(&fx.root);
        let mut ledger = store.read_ledger().unwrap();
        assert!(ledger.fast_lane_prepare_fences.is_empty());
        let receipt = crate::execution_actions::execute_fastlane_primary_for_chain(
            &store.read_genesis().unwrap(),
            &mut ledger,
            &transaction,
            7,
        );
        assert!(receipt.accepted, "{receipt:?}");
        assert_eq!(receipt.code, "fastlane_control_applied");
        assert_eq!(ledger.fast_lane_prepare_fences, vec![expected_fence]);
        // The fence is the one `ActivateCommittee` later requires.
        assert!(ledger.fast_lane_prepare_fences.iter().any(|fence| {
            fence.committee_epoch == fx.epoch1.domain.committee_epoch && fence.policy_epoch == 1
        }));
    }

    #[test]
    fn fastswap_control_stop_prepare_refuses_rotated_validator_5_and_four_votes() {
        let fx = fixture("stop-prepare-refuse");
        add_policy_and_tip(&fx, 7);
        let control = prepare_stop(&fx, 1).unwrap();
        // Validator-5's key file holds its rotated key; epoch 1 records the old one.
        let error = sign(&fx, &control, "validator-5").unwrap_err().to_string();
        assert!(
            error.contains("does not match the active FastSwap committee"),
            "{error}"
        );
        let mut votes = (0..4)
            .map(|index| sign(&fx, &control, &format!("validator-{index}")).unwrap())
            .collect::<Vec<_>>();
        let error = assemble(&fx, &control, votes.clone())
            .unwrap_err()
            .to_string();
        assert!(error.contains("4 valid votes, quorum is 5"), "{error}");

        // A hand-built four-vote certificate is refused by consensus admission.
        let action: FastLaneControlActionV1 = read_json_file(&control, "control").unwrap();
        let four = FastLaneControlCertificateV1 {
            action: action.clone(),
            votes: votes
                .iter()
                .map(|path| read_json_file(path, "vote").unwrap())
                .collect(),
        };
        let mut ledger = NodeStore::new(&fx.root).read_ledger().unwrap();
        let before = ledger.clone();
        assert!(execute_fastlane_control(&mut ledger, &four, 7).is_err());
        assert_eq!(ledger, before);

        // Validator-5's vote signed with its rotated key, bypassing vote-sign.
        let mut vote = FastLaneControlVoteV1 {
            committee: fx.epoch1.domain.clone(),
            action_digest: action.digest().unwrap(),
            validator_id: "validator-5".to_string(),
            signature: Vec::new(),
        };
        vote.signature = ml_dsa_65_sign_with_context(
            &private_key(&fx, "validator-5"),
            &vote.signing_bytes().unwrap(),
            FASTLANE_CONTROL_CONTEXT_V1,
        )
        .unwrap();
        let v5_file = fx.root.join("validator-5.rotated-stop-vote.json");
        write_json_file(&v5_file, &vote).unwrap();
        votes.push(v5_file);
        let error = assemble(&fx, &control, votes).unwrap_err().to_string();
        assert!(
            error.contains("vote from validator-5 does not verify"),
            "{error}"
        );
        let mut with_v5 = four;
        with_v5.votes.push(vote);
        with_v5
            .votes
            .sort_by(|left, right| left.validator_id.cmp(&right.validator_id));
        assert!(execute_fastlane_control(&mut ledger, &with_v5, 7).is_err());
        assert_eq!(ledger, before);
        assert!(!fx.root.join("control-certificate.json").exists());
    }

    #[test]
    fn fastswap_control_vote_sign_refuses_second_different_vote_for_same_key() {
        let fx = fixture("equivocation");
        anchor_drained_checkpoint(&fx);
        let (epoch2, root) = registry_root(&fx);
        let control = prepare_activation(&fx, 2, &root).unwrap();
        sign(&fx, &control, "validator-0").unwrap();

        // An admissible but different epoch-2 committee: validators 0-4 only.
        let FastLaneControlActionV1::ActivateCommittee {
            final_checkpoint, ..
        } = read_json_file(&control, "control").unwrap()
        else {
            panic!("activate committee");
        };
        let mut other = epoch2.clone();
        other.validators.pop();
        other.domain.validator_count = 5;
        other.domain.quorum = 4;
        other.domain.committee_root = other.computed_root().unwrap();
        let other_control = fx.root.join("other-activate.json");
        write_json_file(
            &other_control,
            &FastLaneControlActionV1::ActivateCommittee {
                committee: other,
                final_checkpoint,
            },
        )
        .unwrap();
        let error = sign(&fx, &other_control, "validator-0")
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("already signed a different activate_committee"),
            "{error}"
        );
        sign(&fx, &other_control, "validator-1").expect("an unrelated key may still sign");
        assert!(fx
            .root
            .join(FASTSWAP_CONTROL_SIGNING_STATE_DIR)
            .join("validator-0-epoch1-activate_committee-2.json")
            .exists());
    }
}
