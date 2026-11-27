//! The single PFSH02 compatibility transfer. All amounts here are disposable fixture units.
use super::*;
use postfiat_privacy_binius::{
    verify_payment, MerkleTree24, VerificationContext, W1_ARCHIVAL_BUDGET_BYTES,
};
use postfiat_types::PqFixtureState;
use std::sync::OnceLock;

pub(super) use postfiat_types::{
    PQ_REHEARSAL_CHAIN_ID as CHAIN_ID, PQ_REHEARSAL_GENESIS_HASH as GENESIS_HASH,
};
const LEDGER_DIGEST: &str = "481c8a8003e8385de5ef0c2fb32eb57136e13b655dfd2221eda3a8853bf9d91ec2de009f09dc1ace2ba1739ef1654c51";
const HOP_CONTEXT: &str = "9916d8d7db2bded05446548dceeca76c29ed49253c9466fc3c477705cc4c999ac76d187cf46704ac6130b85ce63e18f9";
const PRE_ROOT: &str = "1194a390c1710bdc9567ea92c9133802097036646a5fa0342c41ae2ba8c6b2db95f032d950c2607d81c5d291808826da";
// 98 background notes valued 500..597 and two mint outputs valued 700 and 290.
const RESERVE: u64 = 54_743;
const LEDGER: &[u8] =
    include_bytes!("../../../tools/pft-binius-shielded-pq/payments/ledger_hop1.json");

fn fixture_error(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}

struct Fixture {
    initial: PqFixtureState,
    tree: MerkleTree24,
}

fn fixture() -> Result<&'static Fixture, &'static str> {
    static BASE: OnceLock<Result<Fixture, &'static str>> = OnceLock::new();
    BASE.get_or_init(|| {
        #[derive(Deserialize)]
        struct Snapshot {
            coins: Vec<String>,
            anchors: Vec<String>,
            spent_sn: Vec<String>,
            leaf_count_override: Option<u64>,
        }
        if bytes_to_hex(&Sha384::digest(LEDGER)) != LEDGER_DIGEST {
            return Err("pq_fixture_prestate_digest");
        }
        let snapshot: Snapshot =
            serde_json::from_slice(LEDGER).map_err(|_| "pq_fixture_snapshot")?;
        if snapshot.coins.len() != 100
            || snapshot.spent_sn.len() != 2
            || snapshot.leaf_count_override.is_some()
        {
            return Err("pq_fixture_snapshot");
        }
        let leaves = snapshot
            .coins
            .iter()
            .map(|s| {
                hex_to_bytes(s)
                    .ok()
                    .and_then(|v| v.try_into().ok())
                    .ok_or("pq_fixture_commitment")
            })
            .collect::<Result<Vec<[u8; 48]>, _>>()?;
        let tree = MerkleTree24::from_owned_leaves(leaves);
        let root = bytes_to_hex(&tree.root());
        if root != PRE_ROOT || snapshot.anchors.last() != Some(&root) {
            return Err("pq_fixture_prestate_root");
        }
        postfiat_privacy_binius::initialize();
        Ok(Fixture {
            initial: PqFixtureState {
                commitments: snapshot.coins,
                spent_serials: snapshot.spent_sn,
                root,
                reserve: RESERVE,
            },
            tree,
        })
    })
    .as_ref()
    .map_err(|e| *e)
}

pub(super) fn genesis_shielded_state(genesis: &Genesis) -> io::Result<ShieldedState> {
    let mut state = ShieldedState::empty();
    if genesis.chain_id == CHAIN_ID {
        if genesis_hash(genesis) != GENESIS_HASH {
            return Err(fixture_error("pq_fixture_wrong_genesis"));
        }
        state.pq_fixture = Some(fixture().map_err(fixture_error)?.initial.clone());
    }
    Ok(state)
}

pub(super) fn check_state_domain(genesis_hash: &str, state: &ShieldedState) -> io::Result<()> {
    if state.pq_pool_v1.is_some()
        && genesis_hash != GENESIS_HASH
        && genesis_hash != postfiat_types::PQ_DEVNET_GENESIS_HASH
    {
        return Err(fixture_error("pq_pool_inactive"));
    }
    if state.pq_fixture.is_some() != (genesis_hash == GENESIS_HASH) {
        return Err(fixture_error("pq_fixture_state_genesis_mismatch"));
    }
    if state.pq_fixture.is_some() {
        fixture().map_err(fixture_error)?;
    }
    Ok(())
}

pub(super) fn apply(
    genesis_hash: &str,
    state: &mut ShieldedState,
    payment_hex: &str,
) -> Result<String, &'static str> {
    if genesis_hash != GENESIS_HASH {
        return Err("pq_fixture_wrong_genesis");
    }
    let pool = state
        .pq_fixture
        .as_mut()
        .ok_or("pq_fixture_missing_prestate")?;
    if payment_hex.len() > W1_ARCHIVAL_BUDGET_BYTES * 2 {
        return Err("pq_fixture_encoding");
    }
    let base = fixture()?;
    let wire = hex_to_bytes(payment_hex).map_err(|_| "pq_fixture_encoding")?;
    let hop =
        verify_payment(&wire, VerificationContext::StandaloneFixture).map_err(|e| match e {
            postfiat_privacy_binius::VerifyError::Encoding => "pq_fixture_encoding",
            postfiat_privacy_binius::VerifyError::InvalidPublicFlow => "pq_fixture_invalid_flow",
            postfiat_privacy_binius::VerifyError::InvalidProof => "pq_fixture_invalid_proof",
        })?;
    let id = bytes_to_hex(&hop.tx_ctx);
    if id != HOP_CONTEXT || hop.v_mint != 0 || hop.v_unshield != 0 {
        return Err("pq_fixture_wrong_payment");
    }
    let serials = [bytes_to_hex(&hop.sn0), bytes_to_hex(&hop.sn1)];
    let outputs = [bytes_to_hex(&hop.coin_out0), bytes_to_hex(&hop.coin_out1)];
    if bytes_to_hex(&hop.anchor) != base.initial.root {
        return Err("pq_fixture_unknown_anchor");
    }
    if serials[0] == serials[1] || serials.iter().any(|s| pool.spent_serials.contains(s)) {
        return Err("pq_fixture_spent_serial");
    }
    if hop.coin_out0 == [0; 48]
        || hop.coin_out1 == [0; 48]
        || outputs[0] == outputs[1]
        || outputs.iter().any(|c| pool.commitments.contains(c))
    {
        return Err("pq_fixture_invalid_commitment");
    }
    if pool.commitments.len() > (1 << 24) - 2 || *pool != base.initial {
        return Err("pq_fixture_invalid_prestate");
    }
    let reserve = pool
        .reserve
        .checked_sub(hop.fee)
        .ok_or("pq_fixture_reserve")?;
    let mut tree = base.tree.clone();
    tree.append(hop.coin_out0);
    tree.append(hop.coin_out1);
    pool.root = bytes_to_hex(&tree.root());
    pool.spent_serials.extend(serials);
    pool.commitments.extend(outputs);
    pool.reserve = reserve;
    Ok(id)
}

pub(super) fn check_batch_domain(genesis: &Genesis, batch: &ShieldedActionBatch) -> io::Result<()> {
    if batch
        .actions
        .iter()
        .any(|a| matches!(a, ShieldedAction::PqFixtureHop { .. }))
    {
        if genesis_hash(genesis) != GENESIS_HASH {
            return Err(fixture_error("pq_fixture_wrong_genesis"));
        }
        if batch.actions.len() != 1 {
            return Err(fixture_error("pq_fixture_requires_single_action"));
        }
    }
    Ok(())
}

/// A failed fixture proof invalidates even a non-atomic block.
pub(super) fn ensure_valid(batch: &ShieldedActionBatch, receipts: &[Receipt]) -> io::Result<()> {
    if matches!(
        batch.actions.as_slice(),
        [ShieldedAction::PqFixtureHop { .. }]
    ) {
        match receipts {
            [receipt] if receipt.accepted => (),
            [receipt] => return Err(fixture_error(&receipt.code)),
            _ => return Err(fixture_error("pq_fixture_missing_receipt")),
        }
    }
    Ok(())
}
