use crate::{
    keys::{self, Network},
    storage::private_file,
    tree::{digest, History},
    Result,
};
use pft_binius_shielded::{
    payment::{create_profiled_tx, ShieldedWallet, SpendInput},
    shake256::nullifier,
};
use postfiat_crypto_provider::address_from_public_key;
use postfiat_privacy_binius::{sign_l1_shield, verify_l1, ShieldOutputIntent};
use postfiat_types::pq_payment::{
    self as pq, Mode, PqAsset, PqPair, PqPayment, Terms, PQ_CAPACITY,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Read},
    path::Path,
    time::Instant,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Point {
    pub network: Network,
    pub height: u64,
    pub block_hash: String,
    pub state_root: String,
    pub pairs: u64,
    pub last_page: String,
    pub assets: BTreeMap<String, PqAsset>,
}
impl Point {
    pub fn validate(&self) -> Result<()> {
        self.network.genesis()?;
        digest(&self.state_root)?;
        if self.height != 0 || self.block_hash != "genesis" {
            digest(&self.block_hash)?;
        }
        if self.pairs > 0 {
            digest(&self.last_page)?;
        } else if !self.last_page.is_empty() {
            return Err("invalid empty checkpoint".into());
        }
        for (asset, a) in &self.assets {
            digest(asset)?;
            if Some(&hex::encode(a.writable.frontier_root()?)) != a.writable.anchors.back() {
                return Err("invalid finalized frontier".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Recovery {
    pub version: u8,
    seed: String,
    pub account: u32,
    pub first: u32,
    pub count: u32,
    pub birthday: Point,
}
#[derive(Serialize, Deserialize)]
struct Note {
    index: u32,
    key: u32,
    value: u64,
    blind: String,
    epoch: u64,
    asset: String,
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
enum Phase {
    Prepared,
    // Older wallets may already have disclosed their pending proof.
    #[default]
    Submitted,
    Final,
    Replaced,
}

#[derive(Serialize, Deserialize)]
struct Pending {
    payment: PqPayment,
    asset: String,
    delta: i128,
    serials: [String; 2],
    #[serde(default)]
    phase: Phase,
    #[serde(default)]
    conflict_final: bool,
    #[serde(default)]
    shared_input: Option<String>,
    #[serde(default)]
    order_id: Option<String>,
}
#[derive(Serialize, Deserialize)]
pub struct Wallet {
    pub recovery: Recovery,
    pub point: Point,
    trees: BTreeMap<String, BTreeMap<u64, History>>,
    notes: BTreeMap<String, Note>,
    // Old wallet file format. Imported conservatively on open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pending: Option<Pending>,
    #[serde(default)]
    attempts: BTreeMap<String, Pending>,
    #[serde(default)]
    latest: Option<String>,
}
impl Recovery {
    pub fn new(birthday: Point, account: u32, first: u32, count: u32) -> Result<Self> {
        let mut seed = [0; 48];
        OsRng.fill_bytes(&mut seed);
        let r = Self {
            version: 1,
            seed: hex::encode(seed),
            birthday,
            account,
            first,
            count,
        };
        r.validate()?;
        Ok(r)
    }
    fn validate(&self) -> Result<()> {
        if self.version != 1
            || !(1..=256).contains(&self.count)
            || self.first.checked_add(self.count - 1).is_none()
        {
            return Err("invalid recovery range or version".into());
        }
        digest(&self.seed)?;
        self.birthday.validate()
    }
    fn key(&self, index: u32) -> Result<ShieldedWallet> {
        if index < self.first || index - self.first >= self.count {
            return Err("address outside backed-up range".into());
        }
        Ok(keys::derive(
            &digest(&self.seed)?,
            &self.birthday.network,
            self.account,
            index,
        )?)
    }
}
impl Wallet {
    pub fn restore(r: Recovery) -> Result<Self> {
        r.validate()?;
        let mut trees = BTreeMap::new();
        for (asset, state) in &r.birthday.assets {
            trees.insert(
                asset.clone(),
                BTreeMap::from([(
                    state.epoch,
                    History {
                        start: state.writable.clone(),
                        leaves: vec![],
                        terminal: None,
                    },
                )]),
            );
        }
        Ok(Self {
            point: r.birthday.clone(),
            recovery: r,
            trees,
            notes: Default::default(),
            pending: None,
            attempts: BTreeMap::new(),
            latest: None,
        })
    }
    pub fn address(&self, index: u32) -> Result<String> {
        Ok(keys::address(
            &self.point.network,
            &self.recovery.key(index)?.stealth_address(),
        )?)
    }
    pub fn import_pending(&mut self) -> Result<()> {
        if let Some(pending) = self.pending.take() {
            let action = postfiat_types::ShieldedAction::PqPaymentV1(pending.payment.clone());
            let hash = postfiat_types::private_action_hash(&action)?;
            self.latest = Some(hash.clone());
            self.attempts.insert(hash, pending);
        }
        Ok(())
    }
    pub fn balance(&self) -> Value {
        let mut totals = BTreeMap::<&str, i128>::new();
        for n in self.notes.values() {
            *totals.entry(&n.asset).or_default() += i128::from(n.value);
        }
        let delta = self
            .latest
            .as_ref().and_then(|id| self.attempts.get(id))
            .filter(|p| matches!(p.phase, Phase::Prepared | Phase::Submitted))
            .map(|p| json!({"asset":p.asset,"delta":p.delta,"after_pending":totals.get(p.asset.as_str()).copied().unwrap_or(0)+p.delta}));
        let attempts: Vec<_> = self
            .attempts
            .iter()
            .map(|(id, p)| json!({"action_hash":id,"phase":p.phase,"asset":p.asset,
                "unknown_order":!p.conflict_final && p.order_id.is_none() && p.payment.parts().is_ok_and(|parts| parts.0.mode != Mode::Shield)}))
            .collect();
        json!({"attempts":attempts,"finalized":totals,"pending":delta,"height":self.point.height,"pairs":self.point.pairs,"keys":self.recovery.count})
    }
    pub fn sync(&mut self, target: Point, rows: &Path) -> Result<Value> {
        self.import_pending()?;
        target.validate()?;
        if target.network != self.point.network
            || target.height < self.point.height
            || (target.height == self.point.height
                && (target.state_root != self.point.state_root
                    || target.block_hash != self.point.block_hash))
        {
            return Err("checkpoint identity mismatch".into());
        }
        let keys = (self.recovery.first..=self.recovery.first + (self.recovery.count - 1))
            .map(|i| Ok((i, self.recovery.key(i)?)))
            .collect::<Result<Vec<_>>>()?;
        let mut scanned = 0;
        let mut trial_time = std::time::Duration::ZERO;
        let mut pages = 0;
        // Stage only changes since the checkpoint. A truncated page or invalid
        // root must not release reservations, even for an in-memory caller.
        let mut point = self.point.clone();
        let mut appended = BTreeMap::<(String, u64), History>::new();
        let mut note_changes = BTreeMap::new();
        let mut resolutions = BTreeMap::new();
        let mut reader = BufReader::new(private_file(rows)?);
        let mut line = String::new();
        while {
            line.clear();
            reader.by_ref().take(1_048_577).read_line(&mut line)? != 0
        } {
            if line.len() > 1_048_576 {
                return Err("oversize public page".into());
            }
            let pairs: Vec<PqPair> = serde_json::from_str(&line)?;
            if pairs.is_empty() {
                return Err("empty intermediate page".into());
            }
            let last =
                postfiat_rpc_sdk::validate_pq_pairs(&pairs, &point.last_page, target.height)?;
            for pair in pairs {
                if pair.height <= self.recovery.birthday.height
                    || pair.height < point.height
                    || point.pairs >= target.pairs
                {
                    return Err("pair out of coverage".into());
                }
                let terminal = crate::tree::advance(
                    point.assets.entry(pair.asset.clone()).or_default(),
                    &pair,
                )?;
                let base_count = self
                    .trees
                    .get(&pair.asset)
                    .and_then(|epochs| epochs.get(&pair.epoch))
                    .map_or(0, History::count);
                let history = appended
                    .entry((pair.asset.clone(), pair.epoch))
                    .or_default();
                let position = base_count + history.leaves.len();
                if position != pair.first_index as usize || position > PQ_CAPACITY as usize - 2 {
                    return Err("output position mismatch".into());
                }
                for serial in &pair.serials {
                    if self.notes.contains_key(serial) || note_changes.contains_key(serial) {
                        note_changes.insert(serial.clone(), None);
                    }
                }
                for (id, attempt) in &self.attempts {
                    if attempt.conflict_final
                        || resolutions.contains_key(id)
                        || !attempt.serials.iter().any(|sn| pair.serials.contains(sn))
                    {
                        continue;
                    }
                    let (_, _, wire) = attempt.payment.parts()?;
                    let matches = attempt.serials == pair.serials
                        && pair.outputs[0].commitment == hex::encode(&wire[208..256])
                        && pair.outputs[1].commitment == hex::encode(&wire[256..304]);
                    let phase = if matches {
                        Phase::Final
                    } else {
                        Phase::Replaced
                    };
                    resolutions.insert(id.clone(), phase);
                }
                for output in &pair.outputs {
                    let coin = digest(&output.commitment)?;
                    let envelope = output.envelope()?.try_into().map_err(|_| "envelope size")?;
                    let start = Instant::now();
                    for (i, k) in &keys {
                        if let Some((value, blind)) = k.try_scan_output(&coin, &envelope) {
                            if value > 0 {
                                note_changes.insert(
                                    hex::encode(nullifier(&k.spend_secret, &coin)),
                                    Some(Note {
                                        index: (base_count + history.leaves.len()) as u32,
                                        key: *i,
                                        value,
                                        blind: hex::encode(blind),
                                        epoch: pair.epoch,
                                        asset: pair.asset.clone(),
                                    }),
                                );
                            }
                        }
                    }
                    trial_time += start.elapsed();
                    scanned += 1;
                    history.leaves.push(output.commitment.clone());
                }
                history.terminal = terminal;
                point.height = pair.height;
                point.pairs += 1;
            }
            point.last_page = last;
            pages += 1;
        }
        if target.pairs != point.pairs || target.last_page != point.last_page {
            return Err("incomplete public history".into());
        }
        if target.assets.len() != point.assets.len() {
            return Err("asset coverage mismatch".into());
        }
        for (asset, a) in &target.assets {
            let rebuilt = point.assets.get(asset).ok_or("missing asset")?;
            if rebuilt.epoch != a.epoch
                || rebuilt.writable != a.writable
                || rebuilt.frozen_epochs_root != a.frozen_epochs_root
            {
                return Err("finalized root mismatch".into());
            }
        }
        for ((asset, epoch), mut addition) in appended {
            let history = self
                .trees
                .entry(asset)
                .or_default()
                .entry(epoch)
                .or_default();
            history.leaves.append(&mut addition.leaves);
            history.terminal = addition.terminal;
        }
        for (serial, change) in note_changes {
            if let Some(note) = change {
                self.notes.insert(serial, note);
            } else {
                self.notes.remove(&serial);
            }
        }
        for (id, phase) in resolutions {
            let attempt = self.attempts.get_mut(&id).expect("scanned wallet attempt");
            attempt.phase = phase;
            attempt.conflict_final = true;
        }
        self.point = target;
        Ok(
            json!({"height":self.point.height,"keys":keys.len(),"pages":pages,"outputs":scanned,"trial_seconds":trial_time.as_secs_f64(),"lag_at_snapshot":0}),
        )
    }
    pub fn submitted(&mut self, hash: &str) -> Result<()> {
        let attempt = self.attempts.get_mut(hash).ok_or("unknown payment")?;
        if attempt.phase == Phase::Prepared {
            attempt.phase = Phase::Submitted;
        }
        Ok(())
    }
    pub fn prepared(&self, hash: Option<&str>) -> Result<Value> {
        let id = hash
            .or(self.latest.as_deref())
            .ok_or("no pending payment")?;
        let p = self.attempts.get(id).ok_or("unknown payment")?;
        Ok(serde_json::to_value(
            postfiat_types::ShieldedAction::PqPaymentV1(p.payment.clone()),
        )?)
    }
}

#[derive(Deserialize)]
pub struct Order {
    #[serde(default)]
    pub replace: Option<String>,
    #[serde(default)]
    pub different_payment: bool,
    pub mode: Mode,
    pub asset: String,
    pub amount: u64,
    pub fee: u64,
    pub recipient: Option<String>,
    pub payer: Option<String>,
    pub account_key: Option<std::path::PathBuf>,
    pub sequence: Option<u64>,
}
#[derive(Deserialize)]
struct SigningKey {
    public_key_hex: String,
    private_key_hex: String,
}
impl Wallet {
    pub fn prove(&mut self, o: Order) -> Result<Value> {
        self.import_pending()?;
        let parent = o
            .replace
            .as_ref()
            .map(|id| self.attempts.get(id).ok_or("unknown replacement payment"))
            .transpose()?;
        if let Some(p) = parent {
            if o.mode == Mode::Shield
                || p.payment.parts()?.0.mode == Mode::Shield
                || p.phase != Phase::Submitted
                || p.asset != o.asset
            {
                return Err("replacement requires a submitted spend of the same asset".into());
            }
        }
        // Retrying the same CLI command after a lost reply must not silently
        // create a second transfer from other funded notes.
        let order_id = if o.mode == Mode::Shield {
            None
        } else {
            let terms = serde_json::to_vec(&(o.mode, &o.asset, o.amount, &o.recipient))?;
            Some(postfiat_crypto_provider::hash_hex(
                "postfiat.wallet.payment-order.v1",
                &terms,
            ))
        };
        if order_id.is_some() && !o.different_payment {
            for (id, p) in &self.attempts {
                if p.asset == o.asset && !p.conflict_final && p.order_id.is_none()
                    && p.payment.parts()?.0.mode != Mode::Shield
                    && Some(id) != o.replace.as_ref()
                    && !parent.is_some_and(|parent| parent.shared_input.is_some() && parent.shared_input == p.shared_input)
                {
                    return Err(format!("Imported spend {id} is unresolved. Retry its bytes, select it for a replacement sharing an input, or confirm this is a different payment.").into());
                }
            }
        }
        if let Some((id, _)) = self.attempts.iter().find(|(id, attempt)| {
            order_id.is_some()
                && attempt.order_id == order_id
                && !attempt.conflict_final
                && Some(*id) != o.replace.as_ref()
                && !parent.is_some_and(|p| {
                    p.shared_input.is_some() && p.shared_input == attempt.shared_input
                })
        }) {
            return Err(format!(
                "matching payment {id} is unresolved; retry its bytes or select it for replacement"
            )
            .into());
        }
        if o.amount == 0 || o.fee == 0 {
            return Err("amount and network fee must be positive".into());
        }
        let asset = digest(&o.asset)?;
        if hex::encode(asset) != o.asset {
            return Err("asset ID must use canonical lowercase hex".into());
        }
        let issued = asset != [0; 48];
        let public = o.mode == Mode::Shield;
        let public_fee = if issued && public { o.fee } else { 0 };
        let inner_fee = o.fee - public_fee;
        let net = &self.point.network;
        let chain = pq::Chain {
            id: &net.chain_id,
            genesis: net.genesis()?,
            protocol: net.protocol_version,
        };
        let own = self.recovery.key(self.recovery.first)?.stealth_address();
        let recipient = match o.mode {
            Mode::Internal => keys::decode(
                net,
                o.recipient.as_deref().ok_or("missing receive address")?,
            )?,
            _ => own.clone(),
        };
        let dest = if o.mode == Mode::Exit {
            pq::destination(o.recipient.as_deref().ok_or("missing public recipient")?)?
        } else {
            [0; 32]
        };
        let signing =
            match (&o.account_key, &o.payer, o.sequence, public) {
                (Some(path), Some(payer), Some(seq), true) if seq > 0 => {
                    let key: SigningKey = serde_json::from_reader(private_file(path)?.take(32768))
                        .map_err(|_| "invalid local signing key")?;
                    let pk = hex::decode(key.public_key_hex)?;
                    if address_from_public_key(&pk) != *payer {
                        return Err("public payer mismatch".into());
                    }
                    Some((
                        pq::Account {
                            id: pq::destination(payer)?[12..].try_into()?,
                            sequence: seq,
                            public_key: Box::new(pk.try_into().map_err(|_| "invalid payer key")?),
                        },
                        hex::decode(key.private_key_hex)?,
                    ))
                }
                (None, None, None, false) => None,
                _ => return Err(
                    "supply exactly the required public payer, signing key path and next sequence"
                        .into(),
                ),
            };
        let mint = if o.mode == Mode::Shield { o.amount } else { 0 };
        let exit = if o.mode == Mode::Exit { o.amount } else { 0 };
        let spend = if mint > 0 {
            0
        } else {
            o.amount.checked_add(inner_fee).ok_or("amount overflow")?
        };
        let mut selected = vec![];
        let mut epoch = self.point.assets.get(&o.asset).map_or(0, |a| a.epoch);
        if spend > 0 {
            let mut groups = BTreeMap::<u64, Vec<&Note>>::new();
            let required = parent
                .map(|p| {
                    p.shared_input
                        .as_deref()
                        .or_else(|| {
                            p.serials
                                .iter()
                                .filter_map(|sn| self.notes.get(sn).map(|n| (n.value, sn.as_str())))
                                .max_by_key(|(value, _)| *value)
                                .map(|(_, sn)| sn)
                        })
                        .ok_or("replacement input is spent")
                })
                .transpose()?;
            let mut shared = None;
            for (sn, n) in self.notes.iter().filter(|(_, n)| n.asset == o.asset) {
                if required == Some(sn.as_str()) {
                    shared = Some(n);
                } else if !self.attempts.iter().any(|(id, p)| {
                    !p.conflict_final
                        && Some(id) != o.replace.as_ref()
                        && !(required.is_some() && p.shared_input.as_deref() == required)
                        && p.serials.contains(sn)
                }) {
                    groups.entry(n.epoch).or_default().push(n);
                }
            }
            if let Some(n) = shared {
                groups.entry(n.epoch).or_default();
            }
            for (e, mut notes) in groups {
                notes.sort_by_key(|n| std::cmp::Reverse(n.value));
                if required.is_some() {
                    let Some(n) = shared.filter(|n| n.epoch == e) else {
                        continue;
                    };
                    notes.insert(0, n);
                }
                notes.truncate(2);
                if notes.iter().map(|n| u128::from(n.value)).sum::<u128>() >= u128::from(spend) {
                    if notes[0].value >= spend {
                        notes.truncate(1);
                    }
                    selected = notes;
                    epoch = e;
                    break;
                }
            }
            if selected.is_empty() {
                return Err("insufficient funds in two notes sharing an epoch".into());
            }
        }
        let empty = History::default();
        let h = self
            .trees
            .get(&o.asset)
            .and_then(|t| t.get(&epoch))
            .unwrap_or(&empty);
        let opening_root = pq::PqEpoch::default();
        let root = h
            .terminal
            .as_ref()
            .or_else(|| {
                self.point
                    .assets
                    .get(&o.asset)
                    .and_then(|a| a.writable.anchors.back())
            })
            .unwrap_or_else(|| opening_root.anchors.back().unwrap());
        let tree = h.tree(digest(root)?);
        let mut inputs = [SpendInput::Dummy, SpendInput::Dummy];
        let mut total = u128::from(mint);
        for (slot, n) in inputs.iter_mut().zip(selected) {
            *slot = SpendInput::Real {
                leaf_index: n.index as usize,
                spend_secret: self.recovery.key(n.key)?.spend_secret,
                value: n.value,
                r_blind: hex::decode(&n.blind)?
                    .try_into()
                    .map_err(|_| "invalid opening")?,
            };
            total += u128::from(n.value);
        }
        let value = match o.mode {
            Mode::Shield => mint.checked_sub(inner_fee).ok_or("mint below fee")?,
            Mode::Internal => o.amount,
            Mode::Exit => 0,
        };
        let change = u64::try_from(
            total
                .checked_sub(u128::from(value) + u128::from(exit) + u128::from(inner_fee))
                .ok_or("invalid payment balance")?,
        )?;
        let mut outs = [(recipient, value), (own, change)];
        if OsRng.next_u32() & 1 == 1 {
            outs.swap(0, 1);
        }
        let terms = Terms {
            mode: o.mode,
            asset,
            epoch,
            public_fee,
            account: signing.as_ref().map(|(a, _)| a.clone()),
        };
        let [i0, i1] = inputs;
        let start = Instant::now();
        let (tx, b0, b1) = create_profiled_tx(
            &tree,
            i0,
            i1,
            &outs[0].0,
            outs[0].1,
            &outs[1].0,
            outs[1].1,
            mint,
            exit,
            dest,
            inner_fee,
            None,
            Some(&|raw| terms.statement_context(chain, raw)),
            &mut OsRng,
        )
        .map_err(|_| "local proof construction failed")?;
        let seconds = start.elapsed().as_secs_f64();
        let inner = tx.to_bytes();
        let payment = match signing {
            Some((_, sk)) => sign_l1_shield(
                chain,
                &terms,
                &inner,
                mint,
                inner_fee,
                [
                    ShieldOutputIntent {
                        owner: &outs[0].0.addr,
                        value: outs[0].1,
                        blinder: &b0,
                        envelope: &tx.env0,
                    },
                    ShieldOutputIntent {
                        owner: &outs[1].0.addr,
                        value: outs[1].1,
                        blinder: &b1,
                        envelope: &tx.env1,
                    },
                ],
                &sk,
            )?,
            None => PqPayment::encode(&terms, None, &inner)?,
        };
        verify_l1(&payment, chain).map_err(|_| "local L1 proof verification failed")?;
        let self_pay = o.mode == Mode::Internal
            && (self.recovery.first..=self.recovery.first + (self.recovery.count - 1))
                .any(|i| self.address(i).ok().as_ref() == o.recipient.as_ref());
        let delta = match o.mode {
            Mode::Shield => i128::from(value),
            Mode::Internal if self_pay => -i128::from(inner_fee),
            _ => -i128::from(spend),
        };
        let hash = postfiat_types::private_action_hash(
            &postfiat_types::ShieldedAction::PqPaymentV1(payment.clone()),
        )?;
        let statement_id = hex::encode(terms.statement_context(chain, tx.statement().tx_ctx));
        let serials = [hex::encode(tx.sn0), hex::encode(tx.sn1)];
        let mut shared_input = None;
        if let Some(parent_id) = o.replace {
            let parent = self.attempts.get_mut(&parent_id).ok_or("unknown payment")?;
            let common = serials
                .iter()
                .find(|sn| {
                    parent.serials.contains(sn)
                        && self.notes.contains_key(*sn)
                        && parent
                            .shared_input
                            .as_ref()
                            .is_none_or(|fixed| fixed == *sn)
                })
                .ok_or("replacement has no common input")?
                .clone();
            parent.shared_input = Some(common.clone());
            parent.phase = Phase::Replaced;
            shared_input = Some(common);
        }
        self.latest = Some(hash.clone());
        self.attempts.insert(
            hash.clone(),
            Pending {
                phase: Phase::Prepared,
                conflict_final: false,
                shared_input,
                order_id,
                payment,
                asset: o.asset,
                delta,
                serials,
            },
        );
        Ok(json!({"prove_seconds":seconds,"action_hash":hash,"statement_id":statement_id}))
    }
}

#[cfg(test)]
mod fee_tests {
    use super::*;
    use pft_binius_shielded::shake256::{coin_commit, MerkleTree24};

    fn funded(values: [u64; 2]) -> Wallet {
        let asset = postfiat_types::PQ_REHEARSAL_ISSUE;
        let point = Point {
            network: Network {
                chain_id: "wallet-test".into(),
                genesis_hash: "55".repeat(48),
                protocol_version: 1,
            },
            height: 0,
            block_hash: "genesis".into(),
            state_root: "22".repeat(48),
            pairs: 0,
            last_page: String::new(),
            assets: BTreeMap::new(),
        };
        let mut wallet = Wallet::restore(Recovery::new(point, 0, 0, 1).unwrap()).unwrap();
        let key = wallet.recovery.key(0).unwrap();
        let mut history = History::default();
        let mut tree = MerkleTree24::new();
        for (index, value) in (0u32..).zip(values) {
            let blind = [index as u8; 78];
            let coin = coin_commit(&key.addr, value, &blind);
            tree.append(coin);
            history.leaves.push(hex::encode(coin));
            wallet.notes.insert(
                hex::encode(nullifier(&key.spend_secret, &coin)),
                Note {
                    index,
                    value,
                    key: 0,
                    epoch: 0,
                    asset: asset.into(),
                    blind: hex::encode(blind),
                },
            );
        }
        wallet
            .point
            .assets
            .entry(asset.into())
            .or_default()
            .writable
            .anchors
            .push_back(hex::encode(tree.root()));
        wallet
            .trees
            .entry(asset.into())
            .or_default()
            .insert(0, history);
        wallet
    }

    fn exit(amount: u64, fee: u64) -> Value {
        json!({"mode":"Exit", "asset":postfiat_types::PQ_REHEARSAL_ISSUE, "amount":amount, "fee":fee,
               "recipient":format!("pf{}", "33".repeat(20))})
    }

    #[test]
    fn imported_unknown_order_requires_an_independent_decision() {
        let mut w = funded([5, 5]);
        let first = w.prove(serde_json::from_value(exit(2, 1)).unwrap()).unwrap();
        let id = first["action_hash"].as_str().unwrap();
        let mut state = serde_json::to_value(&w).unwrap();
        let mut pending = state["attempts"].as_object_mut().unwrap().remove(id).unwrap();
        for k in ["order_id","phase"] { pending.as_object_mut().unwrap().remove(k); }
        state["pending"] = pending;
        let mut w: Wallet = serde_json::from_value(state).unwrap();
        let mut order = exit(1, 1);
        assert!(w.prove(serde_json::from_value(order.clone()).unwrap()).unwrap_err()
                   .to_string().contains("different payment"));
        order["different_payment"] = json!(true);
        assert!(w.prove(serde_json::from_value(order).unwrap()).is_ok());
    }

    #[test]
    fn recovery_exit_conflicts_with_both_five_atom_orders() {
        let mut w = funded([5, 5]);
        let mut parents = vec![];
        for amount in [1, 2] {
            let proof = w.prove(serde_json::from_value(exit(amount, 1)).unwrap()).unwrap();
            let id = proof["action_hash"].as_str().unwrap().to_owned();
            w.submitted(&id).unwrap();
            parents.push(id);
        }
        let mut order = exit(4, 6);
        order["replace"] = json!(parents[0]);
        order["also_replace"] = json!(parents[1]);
        let proof = w.prove(serde_json::from_value(order).unwrap()).unwrap();
        let id = proof["action_hash"].as_str().unwrap();
        let state = serde_json::to_value(&w).unwrap();
        assert_eq!(state["attempts"][id]["supersedes"], json!(parents));
        for parent in parents {
            assert!(w.attempts[&parent].serials.iter().any(|sn|
                w.notes.contains_key(sn) && w.attempts[id].serials.contains(sn)));
        }
    }

    #[test]
    fn retired_pending_is_quarantined_in_the_encrypted_wallet() {
        use base64::{Engine, engine::general_purpose::STANDARD};
        use crate::{service::execute, storage::Store};
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("wallet");
        std::fs::create_dir(&home).unwrap();
        std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).unwrap();
        let key = tempfile::NamedTempFile::new_in(tmp.path()).unwrap();
        std::fs::write(key.path(), [42;32]).unwrap();
        let store = Store::new(key.path(), &home).unwrap();
        let mut state = serde_json::to_value(funded([5, 5])).unwrap();
        let wire = include_bytes!("../../privacy_binius/tests/fixtures/pre-profile-2-issued-exit.bin");
        let retired = json!({"payment":{"body":STANDARD.encode(wire)}, "asset":hex::encode(&wire[7..55]),
                           "serials":["11".repeat(48),"22".repeat(48)],"delta":-2});
        state["pending"] = retired.clone();
        let path = home.join("wallet.enc");
        let tag = b"postfiat/pq-wallet/v1/state";
        store.save(&path, tag, &state, true).unwrap();
        let balance = execute(&home,&store,json!({"op":"balance"})).unwrap();
        assert_eq!(balance["finalized"][postfiat_types::PQ_REHEARSAL_ISSUE], 10);
        assert!(!balance["quarantined"].is_null());
        let backup = tmp.path().join("backup");
        execute(&home,&store,json!({"op":"backup","backup":backup})).unwrap();
        let recovery: Value = store.load(&backup,b"postfiat/pq-wallet/v1/recovery").unwrap();
        assert_eq!(recovery, state["recovery"]);
        let saved: Value = store.load(&path,tag).unwrap();
        assert_eq!(saved["quarantined"],retired);
        assert_eq!(saved["notes"],state["notes"]);
        assert!(execute(&home,&store,json!({"op":"prepared"})).is_err());
        assert!(PqPayment::from_bytes(wire.to_vec()).is_err());
    }

    #[test]
    fn issued_spends_select_notes_for_amount_plus_fee_without_a_signer() {
        let asset = postfiat_types::PQ_REHEARSAL_ISSUE;
        let mut wallet = funded([10,2]);
        for mode in [Mode::Internal, Mode::Exit] {
            let recipient = if mode == Mode::Internal {
                wallet.address(0).unwrap()
            } else {
                format!("pf{}", "33".repeat(20))
            };
            let order = |fee| Order {
                different_payment: false,
                replace: None,
                mode,
                asset: asset.into(),
                amount: 10,
                fee,
                recipient: Some(recipient.clone()),
                payer: None,
                sequence: None,
                account_key: None,
            };
            assert!(wallet
                .prove(order(3))
                .unwrap_err()
                .to_string()
                .contains("insufficient funds"));
            let metrics = wallet.prove(order(2)).unwrap();
            let hash = metrics["action_hash"].as_str().unwrap();
            wallet.submitted(hash).unwrap();
            let bytes = wallet.prepared(Some(hash)).unwrap();
            wallet = serde_json::from_slice(&serde_json::to_vec(&wallet).unwrap()).unwrap();
            assert_eq!(wallet.prepared(Some(hash)).unwrap(), bytes);
            assert!(wallet.prove(order(2)).is_err());
            let next = wallet
                .prove(Order {
                    replace: Some(hash.into()),
                    ..order(2)
                })
                .unwrap();
            let pending = wallet
                .attempts
                .remove(next["action_hash"].as_str().unwrap())
                .unwrap();
            let parent = wallet.attempts.remove(hash).unwrap();
            assert!(parent.phase == Phase::Replaced);
            assert!(pending
                .serials
                .contains(parent.shared_input.as_ref().unwrap()));
            assert_eq!(pending.delta, if mode == Mode::Exit { -12 } else { -2 });
            let (terms, sig, inner) = pending.payment.parts().unwrap();
            assert!(terms.account.is_none() && sig.is_none());
            assert_eq!(u64::from_le_bytes(inner[104..112].try_into().unwrap()), 2);
            assert!(pending
                .serials
                .iter()
                .all(|sn| wallet.notes.contains_key(sn)));
        }
        // At the one-atom schedule, the smallest individually exitable note
        // pays one atom to an existing eligible account and one to custody.
        wallet.notes.retain(|_, note| note.value == 2);
        let metrics = wallet
            .prove(Order {
                different_payment: false,
                replace: None,
                mode: Mode::Exit,
                asset: asset.into(),
                amount: 1,
                fee: 1,
                recipient: Some(format!("pf{}", "33".repeat(20))),
                payer: None,
                sequence: None,
                account_key: None,
            })
            .unwrap();
        let attempt = &wallet.attempts[metrics["action_hash"].as_str().unwrap()];
        let (_, _, inner) = attempt.payment.parts().unwrap();
        assert_eq!(u64::from_le_bytes(inner[64..72].try_into().unwrap()), 1);
        assert_eq!(u64::from_le_bytes(inner[104..112].try_into().unwrap()), 1);
        assert_eq!(attempt.delta, -2);
    }
}
