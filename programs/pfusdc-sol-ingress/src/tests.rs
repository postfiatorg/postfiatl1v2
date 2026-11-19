//! The honest world is built with Solana's own `solana-lattice-hash` crate, so these tests
//! compare this file's account hashing and lane arithmetic against an independent
//! implementation, not against itself.
use super::*;
use solana_lattice_hash::lt_hash::LtHash as SolanaLtHash;

const VAULT: [u8; 32] = [7; 32];
const ROUTE: [u8; 32] = [9; 32];
const TOKEN_PROGRAM: [u8; 32] = [6; 32];

#[derive(Clone)]
struct Owned { lamports: u64, executable: bool, owner: [u8; 32], data: Vec<u8> }
impl Owned {
    fn gone() -> Self { Self { lamports: 0, executable: false, owner: [0; 32], data: vec![] } }
    fn state(&self) -> AccountState<'_> { AccountState { lamports: self.lamports, executable: self.executable, owner: self.owner, data: &self.data } }
}
#[derive(Clone)]
struct Change { address: [u8; 32], old: Owned, new: Owned }

/// Solana's hash of one account, through Solana's crate.
fn solana_account_hash(address: &[u8; 32], account: &Owned) -> Option<SolanaLtHash> {
    if account.lamports == 0 { return None; }
    let mut hasher = blake3::Hasher::new();
    hasher.update(&account.lamports.to_le_bytes());
    hasher.update(&account.data);
    hasher.update(&[u8::from(account.executable)]);
    hasher.update(&account.owner);
    hasher.update(address);
    Some(SolanaLtHash::with(&hasher))
}

fn receipt_data(route: [u8; 32], recipient: [u8; 20], amount: u64, nonce: [u8; 32], depositor: [u8; 32]) -> Vec<u8> {
    let mut data = RECEIPT_MAGIC.to_vec();
    data.extend_from_slice(&route); data.extend_from_slice(&recipient); data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&nonce); data.extend_from_slice(&depositor);
    data
}
fn receipt(address: u8, amount: u64) -> Change {
    Change { address: [address; 32], old: Owned::gone(),
        new: Owned { lamports: 1_809_600, executable: false, owner: VAULT, data: receipt_data(ROUTE, [0x33; 20], amount, [address; 32], [0x44; 32]) } }
}
fn token_transfer(address: u8, before: u64, after: u64) -> Change {
    let account = |amount: u64| { let mut data = vec![address; 165]; data[64..72].copy_from_slice(&amount.to_le_bytes());
        Owned { lamports: 2_039_280, executable: false, owner: TOKEN_PROGRAM, data } };
    Change { address: [address; 32], old: account(before), new: account(after) }
}

struct World { parent_parent: [u8; 32], lt_before: SolanaLtHash, changes: Vec<Change>, sig_count: u64 }
impl World {
    fn new(changes: Vec<Change>) -> Self {
        // A non-trivial starting sum: the rest of the chain's accounts.
        let mut lt_before = SolanaLtHash::identity();
        for seed in 0u8..5 {
            lt_before.mix_in(&solana_account_hash(&[200 + seed; 32], &Owned { lamports: 5 + seed as u64, executable: false, owner: [1; 32], data: vec![seed; 40] }).unwrap());
        }
        for change in &changes { if let Some(hash) = solana_account_hash(&change.address, &change.old) { lt_before.mix_in(&hash); } }
        Self { parent_parent: [0x11; 32], lt_before, changes, sig_count: 633 }
    }
    fn lt_after(&self) -> SolanaLtHash {
        let mut sum = self.lt_before.clone();
        for change in &self.changes {
            if let Some(hash) = solana_account_hash(&change.address, &change.old) { sum.mix_out(&hash); }
            if let Some(hash) = solana_account_hash(&change.address, &change.new) { sum.mix_in(&hash); }
        }
        sum
    }
    fn bank_hashes(&self) -> ([u8; 32], [u8; 32]) {
        let lanes = |sum: &SolanaLtHash| -> Vec<u8> { sum.0.iter().flat_map(|lane| lane.to_le_bytes()).collect() };
        let hash = |parent: [u8; 32], sigs: u64, blockhash: [u8; 32], sum: &SolanaLtHash| -> [u8; 32] {
            let inner: [u8; 32] = Sha256::new().chain_update(parent).chain_update(sigs.to_le_bytes()).chain_update(blockhash).finalize().into();
            Sha256::new().chain_update(inner).chain_update(lanes(sum)).finalize().into()
        };
        let parent = hash(self.parent_parent, 500, [0x22; 32], &self.lt_before);
        (parent, hash(parent, self.sig_count, [0x23; 32], &self.lt_after()))
    }
    /// The witness an honest prover would send, with `listed` as the changes it claims.
    fn witness_listing(&self, listed: &[Change]) -> Vec<u8> {
        let (parent, bank) = self.bank_hashes();
        let lanes = |sum: &SolanaLtHash| -> Vec<u8> { sum.0.iter().flat_map(|lane| lane.to_le_bytes()).collect() };
        let mut out = Vec::new();
        out.extend_from_slice(&bank); out.extend_from_slice(&VAULT); out.extend_from_slice(&ROUTE);
        out.extend_from_slice(&parent); out.extend_from_slice(&self.sig_count.to_le_bytes()); out.extend_from_slice(&[0x23; 32]);
        out.extend_from_slice(&lanes(&self.lt_after()));
        out.extend_from_slice(&self.parent_parent); out.extend_from_slice(&500u64.to_le_bytes()); out.extend_from_slice(&[0x22; 32]);
        out.extend_from_slice(&lanes(&self.lt_before));
        out.extend_from_slice(&(listed.len() as u32).to_le_bytes());
        for change in listed {
            out.extend_from_slice(&change.address);
            for account in [&change.old, &change.new] {
                out.extend_from_slice(&account.lamports.to_le_bytes()); out.push(u8::from(account.executable));
                out.extend_from_slice(&account.owner); out.extend_from_slice(&(account.data.len() as u32).to_le_bytes()); out.extend_from_slice(&account.data);
            }
        }
        out
    }
    fn witness(&self) -> Vec<u8> { self.witness_listing(&self.changes) }
}

fn honest() -> World { World::new(vec![token_transfer(1, 9_000_000, 4_000_000), token_transfer(2, 0, 5_000_000), receipt(3, 5_000_000), token_transfer(4, 77, 78)]) }

#[test]
fn account_hash_matches_solana_crate() {
    for (address, account) in [([1u8; 32], Owned { lamports: 1, executable: false, owner: [2; 32], data: vec![] }),
                               ([5u8; 32], Owned { lamports: u64::MAX, executable: true, owner: [9; 32], data: (0..=255).cycle().take(5000).collect() })] {
        let ours = account_lt_hash(&address, &account.state()).unwrap();
        let theirs: Vec<u8> = solana_account_hash(&address, &account).unwrap().0.iter().flat_map(|lane| lane.to_le_bytes()).collect();
        assert_eq!(ours.as_slice(), theirs.as_slice());
    }
    assert!(account_lt_hash(&[1; 32], &Owned::gone().state()).is_none());
}

#[test]
fn honest_block_yields_its_receipt() {
    let world = honest();
    let values = verify_state_witness(&world.witness()).unwrap();
    assert_eq!(values.changed_accounts, 4);
    assert_eq!(values.receipts.len(), 1);
    let found = &values.receipts[0];
    assert_eq!((found.address, found.amount, found.pftl_recipient, found.depositor), ([3; 32], 5_000_000, [0x33; 20], [0x44; 32]));
    assert_eq!(found.deposit_id, deposit_id(&VAULT, &[3; 32], &receipt(3, 5_000_000).new.data));
    assert_eq!(values.bank_hash, world.bank_hashes().1);
    assert_eq!(values.encode(), verify_state_witness(&world.witness()).unwrap().encode());
    assert_eq!(values.encode().len(), 136 + 156);
}

#[test]
fn two_deposits_in_one_block_share_one_proof() {
    let world = World::new(vec![receipt(3, 5_000_000), receipt(8, 1_000_000), token_transfer(1, 10, 20)]);
    let values = verify_state_witness(&world.witness()).unwrap();
    assert_eq!(values.receipts.iter().map(|r| r.amount).collect::<Vec<_>>(), vec![5_000_000, 1_000_000]);
    assert_ne!(values.receipts[0].deposit_id, values.receipts[1].deposit_id);
}

fn refused(world: &World, listed: Vec<Change>, expected: Error) {
    assert_eq!(verify_state_witness(&world.witness_listing(&listed)).unwrap_err(), expected);
}

#[test]
fn a_receipt_the_block_did_not_create_is_refused() {
    let world = honest();
    let mut listed = world.changes.clone();
    listed.push(receipt(50, 500_000_000_000));
    refused(&world, listed, Error::DeltaMismatch);
}

#[test]
fn an_inflated_amount_is_refused() {
    let world = honest();
    let mut listed = world.changes.clone();
    listed[2] = receipt(3, 500_000_000_000);
    refused(&world, listed, Error::DeltaMismatch);
}

#[test]
fn a_missing_changed_account_is_refused() {
    let world = honest();
    let listed = world.changes[..3].to_vec();
    refused(&world, listed, Error::DeltaMismatch);
}

#[test]
fn a_false_old_state_is_refused() {
    let world = honest();
    let mut listed = world.changes.clone();
    listed[0].old.lamports -= 1;
    refused(&world, listed, Error::DeltaMismatch);
}

#[test]
fn a_repeated_account_is_refused_even_when_the_sum_balances() {
    // Created, removed, created again: the three entries net to one receipt and the sum
    // balances, so only the duplicate check stops a second credit.
    let world = honest();
    let mut listed = world.changes.clone();
    let created = listed[2].clone();
    listed.push(Change { address: created.address, old: created.new.clone(), new: created.old.clone() });
    listed.push(created);
    refused(&world, listed, Error::DuplicateAccount);
}

#[test]
fn only_new_accounts_of_the_pinned_program_are_receipts() {
    // Same layout, another owner: not a receipt.
    let mut other = receipt(3, 5_000_000); other.new.owner = [8; 32];
    let world = World::new(vec![other, token_transfer(1, 1, 2)]);
    assert_eq!(verify_state_witness(&world.witness()).unwrap_err(), Error::NoReceipt);
    // A receipt that already existed before the block cannot be counted again.
    let mut existing = receipt(3, 5_000_000); existing.old = existing.new.clone(); existing.new.lamports += 1;
    let world = World::new(vec![existing]);
    assert_eq!(verify_state_witness(&world.witness()).unwrap_err(), Error::NoReceipt);
}

#[test]
fn a_new_vault_account_that_is_not_a_receipt_for_this_route_is_refused() {
    for change in [
        { let mut c = receipt(3, 5); c.new.data[8] ^= 1; c },                 // another route binding
        { let mut c = receipt(3, 5); c.new.data[0] ^= 1; c },                 // wrong magic
        { let mut c = receipt(3, 5); c.new.data.push(0); c },                 // wrong length
        { let mut c = receipt(3, 5); c.new.executable = true; c },
        receipt(3, 0),                                                         // zero amount
    ] {
        let world = World::new(vec![change]);
        assert_eq!(verify_state_witness(&world.witness()).unwrap_err(), Error::MalformedVaultAccount);
    }
}

#[test]
fn bank_hash_bindings_are_checked() {
    let world = honest();
    let honest_witness = world.witness();
    let flip = |at: usize| { let mut witness = honest_witness.clone(); witness[at] ^= 1; verify_state_witness(&witness).unwrap_err() };
    assert_eq!(flip(0), Error::BankHashMismatch);                                // claimed bank hash
    assert_eq!(flip(96 + 32), Error::BankHashMismatch);                          // signature count
    assert_eq!(flip(96 + 72 + 5), Error::BankHashMismatch);                      // a lane of the sum after
    assert_eq!(flip(96 + 72 + LT_BYTES + 32), Error::ParentBankHashMismatch);    // parent signature count
    assert_eq!(flip(96 + 72 + LT_BYTES + 72 + 5), Error::ParentBankHashMismatch); // a lane of the sum before
}

#[test]
fn malformed_witnesses_are_refused() {
    let world = honest();
    let witness = world.witness();
    let mut longer = witness.clone(); longer.push(0);
    assert_eq!(verify_state_witness(&longer).unwrap_err(), Error::TrailingBytes);
    for cut in [0, 31, 96, 96 + 72 + LT_BYTES, witness.len() - 1] {
        assert_eq!(verify_state_witness(&witness[..cut]).unwrap_err(), Error::Truncated);
    }
    let mut flagged = World::new(vec![receipt(3, 5)]).witness();
    let flag_at = 96 + 2 * (72 + LT_BYTES) + 4 + 32 + 8;
    flagged[flag_at] = 2;
    assert_eq!(verify_state_witness(&flagged).unwrap_err(), Error::InvalidExecutableFlag);
}

#[test]
fn an_account_closed_in_the_block_leaves_the_sum() {
    let mut closed = token_transfer(1, 5, 0); closed.new = Owned::gone();
    let world = World::new(vec![closed, receipt(3, 5)]);
    assert_eq!(verify_state_witness(&world.witness()).unwrap().changed_accounts, 2);
}

#[test]
fn recorder_layouts_have_exact_lengths_and_epoch() {
    use recorder::*;
    let key = bls12_381::G1Affine::generator().to_compressed();
    let mut batch = b"PFSOLKY1".to_vec();
    batch.extend(19u64.to_le_bytes());
    batch.extend(2u32.to_le_bytes());
    batch.extend(key);
    batch.extend(key);
    assert!(recorded_keys(&batch, 19).unwrap() == vec![key]);
    assert_eq!(recorded_keys(&batch, 20).err(), Some(RecorderError::WrongEpoch));
    let mut invalid = batch.clone();
    invalid[KEY_HEADER_LEN..KEY_HEADER_LEN + KEY_ROW_LEN].fill(0);
    assert_eq!(recorded_keys(&invalid, 19).err(), Some(RecorderError::InvalidKey));
    for cut in [0, KEY_HEADER_LEN - 1, KEY_HEADER_LEN, batch.len() - 1] {
        assert_eq!(recorded_keys(&batch[..cut], 19).err(), Some(RecorderError::Malformed));
    }
    batch.push(0);
    assert_eq!(recorded_keys(&batch, 19).err(), Some(RecorderError::Malformed));

    let mut snapshot = b"PFSOLST1".to_vec();
    snapshot.extend(19u64.to_le_bytes());
    snapshot.extend(100u64.to_le_bytes());
    snapshot.extend(1u32.to_le_bytes());
    let mut row = [0; 120];
    row[..32].fill(1);
    row[32..64].fill(2);
    row[64..72].copy_from_slice(&80u64.to_le_bytes());
    row[72..].copy_from_slice(&key);
    snapshot.extend(row);
    let set = recorded_set(&snapshot, 19).unwrap();
    assert_eq!(set.total_stake, 100);
    assert_eq!(set.validators[0].stake, 80);
    assert!(set.validators[0].pubkey == key);
    assert_eq!(recorded_set(&snapshot[..snapshot.len()-1], 19).err(), Some(RecorderError::Malformed));
    snapshot[TOTAL_STAKE_OFFSET..STAKE_COUNT_OFFSET].copy_from_slice(&79u64.to_le_bytes());
    assert_eq!(recorded_set(&snapshot, 19).err(), Some(RecorderError::InvalidTotalStake));
    snapshot[STAKE_COUNT_OFFSET..STAKE_HEADER_LEN].copy_from_slice(&2u32.to_le_bytes());
    snapshot.extend(row);
    assert_eq!(recorded_set(&snapshot, 19).err(), Some(RecorderError::DuplicateVoteAccount));
}

#[test]
fn known_key_commitment_has_one_encoding_and_binds_every_member() {
    use handoff::{known_key_table_hash, HandoffError};
    let keys = [[1; 48], [2; 48]];
    let expected: [u8; 32] = Sha256::new()
        .chain_update(b"postfiat.pfusdc.solana.known_keys.v1")
        .chain_update([2, 0, 0, 0])
        .chain_update(keys[0])
        .chain_update(keys[1])
        .finalize().into();
    assert_eq!(known_key_table_hash(&keys), Ok(expected));
    assert_ne!(known_key_table_hash(&keys[..1]), Ok(expected));
    assert_ne!(known_key_table_hash(&[keys[0], [3; 48]]), Ok(expected));
    for bad in [[keys[1], keys[0]], [keys[0], keys[0]]] {
        assert_eq!(known_key_table_hash(&bad), Err(HandoffError::NonCanonicalKeyTable));
    }
}

#[test]
fn recorder_requires_a_valid_epoch_schedule_and_complete_witness() {
    use handoff::HandoffError;
    for (slots, flag) in [(0u64, 0u8), (31, 0), (u64::MAX, 1), (32, 2)] {
        let mut input = 0u64.to_le_bytes().to_vec();
        input.extend(slots.to_le_bytes());
        input.push(flag);
        assert_eq!(verify_key_table_witness(&input), Err(HandoffError::InvalidSchedule));
        assert_eq!(verify_handoff_witness(&input), Err(HandoffError::InvalidSchedule));
    }
    assert_eq!(verify_handoff_witness(&[]), Err(HandoffError::Input(DepositError::Truncated)));
}

#[test]
fn capture_mode_checks_a_receiptless_delta() {
    let world = World::new(vec![token_transfer(1, 5, 3), token_transfer(2, 1, 3)]);
    let witness = world.witness();
    assert_eq!(verify_state_witness(&witness), Err(Error::NoReceipt));
    let values = verify_state(&witness, false).unwrap();
    assert_eq!(values.changed_accounts, 2);
    assert!(values.receipts.is_empty());
    let missing = world.witness_listing(&world.changes[..1]);
    assert_eq!(verify_state(&missing, false), Err(Error::DeltaMismatch));
}
