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
    other_shared_input: Option<String>,
    #[serde(default)]
    supersedes: Vec<String>,
    #[serde(default)]
    order_id: Option<String>,
    #[serde(default)]
    review_terms: Option<Value>,
    #[serde(default)]
    migration: Option<String>,
}
// This local record has no conversion to a submittable action.
#[derive(Serialize, Deserialize)]
struct LegacyPending {
    payment: Value,
    asset: String,
    serials: [String; 2],
    delta: i128,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}

impl LegacyPending {
    fn check_retired(&self) -> Result<()> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let body = self.payment["body"]
            .as_str()
            .ok_or("invalid legacy payment")?;
        if body.len() > pq::ARCHIVE_LIMIT.div_ceil(3) * 4 {
            return Err("oversize legacy payment".into());
        }
        let wire = STANDARD.decode(body)?;
        if wire.len() < 72
            || wire.len() > pq::ARCHIVE_LIMIT
            || &wire[..6] != b"PFPQ\x01\x01"
            || !matches!(wire[6], 0 | 2)
            || wire[71] != 1
            || wire[7..55] == [0; 48]
            || hex::encode(&wire[7..55]) != self.asset
        {
            return Err("invalid legacy payment".into());
        }
        for sn in &self.serials {
            digest(sn)?;
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct Wallet {
    pub recovery: Recovery,
    pub point: Point,
    trees: BTreeMap<String, BTreeMap<u64, History>>,
    notes: BTreeMap<String, Note>,
    // Old wallet file format. Imported conservatively on open.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pending: Option<LegacyPending>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    quarantined: Option<LegacyPending>,
    #[serde(default)]
    attempts: BTreeMap<String, Pending>,
    #[serde(default)]
    latest: Option<String>,
    /// Next rotating offset within `[recovery.first .. recovery.first + recovery.count)`
    /// for fresh per-note Shield and change outputs so spends do not reuse one static ML-DSA key.
    #[serde(default)]
    next_change_offset: u32,
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
            quarantined: None,
            attempts: BTreeMap::new(),
            latest: None,
            next_change_offset: 0,
        })
    }
    fn next_own_stealth_address(&mut self) -> Result<pft_binius_shielded::payment::StealthAddress> {
        let count = self.recovery.count.max(1);
        let offset = self.next_change_offset % count;
        self.next_change_offset = offset.wrapping_add(1) % count;
        Ok(self
            .recovery
            .key(self.recovery.first + offset)?
            .stealth_address())
    }
    pub fn address(&self, index: u32) -> Result<String> {
        Ok(keys::address(
            &self.point.network,
            &self.recovery.key(index)?.stealth_address(),
        )?)
    }
    pub fn import_pending(&mut self) -> Result<bool> {
        if let Some(record) = &self.pending {
            let pending: Pending = match serde_json::from_slice(&serde_json::to_vec(record)?) {
                Ok(pending) => pending,
                Err(_) if self.quarantined.is_none() => {
                    record.check_retired()?;
                    self.quarantined = self.pending.take();
                    return Ok(true);
                }
                Err(error) => return Err(error.into()),
            };
            let action = postfiat_types::ShieldedAction::PqPaymentV1(pending.payment.clone());
            let hash = postfiat_types::private_action_hash(&action)?;
            self.latest = Some(hash.clone());
            self.attempts.insert(hash, pending);
            self.pending = None;
            return Ok(true);
        }
        Ok(false)
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
                "unresolved":!p.conflict_final,"supersedes":p.supersedes,"unknown_order":!p.conflict_final && p.order_id.is_none() && p.payment.parts().is_ok_and(|parts| parts.0.mode != Mode::Shield)}))
            .collect();
        json!({"quarantined":self.quarantined.as_ref().map(|p| json!({"asset":p.asset, "message":"Retired issued spend saved locally. Check finalized history and the original order before making another payment."})), "attempts":attempts,"finalized":totals,"pending":delta,"height":self.point.height,"pairs":self.point.pairs,"keys":self.recovery.count})
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
                                    hex::encode(nullifier(&k.r_pk, &coin)),
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
                            break;
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
    pub fn saved_review(&self, id: &str, entered: Option<&Value>) -> Result<Value> {
        use sha2::{Digest, Sha384};
        let p = self.attempts.get(id).ok_or("unknown payment")?;
        let (public, _, inner) = p.payment.parts()?;
        let net = &self.point.network;
        let chain = pq::Chain {
            id: &net.chain_id,
            genesis: net.genesis()?,
            protocol: net.protocol_version,
        };
        let mut raw = Sha384::new();
        raw.update(b"PFT-SH-TXCTX-V3");
        raw.update(&inner[..pq::UNSIGNED_HEADER_LEN]);
        let statement_id = hex::encode(public.statement_context(chain, raw.finalize().into()));
        let terms = if let Some(terms) = &p.review_terms {
            terms.clone()
        } else {
            // Old internal wires expose neither private output. Where possible
            // authenticate the user's recollection with the original order ID.
            let atoms = |start| u64::from_le_bytes(inner[start..start + 8].try_into().unwrap());
            let mut amount = match public.mode {
                Mode::Internal => None,
                Mode::Shield => Some(atoms(56)),
                Mode::Exit => Some(atoms(64)),
            };
            let mut recipient =
                (public.mode == Mode::Exit).then(|| format!("pf{}", hex::encode(&inner[84..104])));
            if public.mode == Mode::Internal && p.order_id.is_some() {
                if let Some(entered) = entered {
                    let n = entered["amount"]
                        .as_str()
                        .ok_or("enter the original amount")?
                        .parse::<u64>()?;
                    recipient = Some(
                        entered["recipient"]
                            .as_str()
                            .ok_or("enter the original recipient")?
                            .to_owned(),
                    );
                    let order = serde_json::to_vec(&(public.mode, &p.asset, n, &recipient))?;
                    if p.order_id.as_deref()
                        != Some(&postfiat_crypto_provider::hash_hex(
                            "postfiat.wallet.payment-order.v1",
                            &order,
                        ))
                    {
                        return Err("amount or recipient differs from the saved order".into());
                    }
                    amount = Some(n);
                }
            }
            json!({"mode":public.mode,"asset":p.asset,"amount":amount.map(|n| n.to_string()),
                "recipient":recipient,"fee":public.public_fee.checked_add(atoms(104)).ok_or("invalid payment fee")?.to_string(),
                "payer":public.account.as_ref().map(|a| format!("pf{}",hex::encode(a.id))),
                "sequence":public.account.as_ref().map(|a| a.sequence.to_string()),"migration":false,
                "needs_original":public.mode == Mode::Internal && p.order_id.is_some() && amount.is_none()})
        };
        Ok(
            json!({"action":self.prepared(Some(id))?,"terms":terms,"statement_id":statement_id,"migration":p.migration}),
        )
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
    #[serde(default)]
    pub also_replace: Option<String>,
    #[serde(default)]
    pub migration: Option<String>,
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
        if o.also_replace.is_some() && (o.replace.is_none() || o.also_replace == o.replace) {
            return Err("select two distinct payments for joint recovery".into());
        }
        let supersedes: Vec<_> = o.replace.iter().chain(&o.also_replace).cloned().collect();
        let mut required = Vec::new();
        let mut parent_inputs = Vec::new();
        for id in &supersedes {
            let p = self.attempts.get(id).ok_or("unknown replacement payment")?;
            if o.mode == Mode::Shield
                || p.payment.parts()?.0.mode == Mode::Shield
                || p.conflict_final
                || p.asset != o.asset
            {
                return Err("replacement requires an unresolved spend of the same asset".into());
            }
            let mut pins: Vec<_> = p
                .shared_input
                .iter()
                .chain(&p.other_shared_input)
                .cloned()
                .collect();
            if pins.is_empty() {
                let sn = p
                    .serials
                    .iter()
                    .filter_map(|sn| self.notes.get(sn).map(|n| (n.value, sn)))
                    .max_by_key(|(value, _)| *value)
                    .ok_or("replacement input is spent")?
                    .1;
                pins.push(sn.clone());
            }
            for sn in &pins {
                if !required.contains(sn) {
                    required.push(sn.clone());
                }
            }
            parent_inputs.push(pins);
        }
        if required.len() > 2 {
            return Err("these groups require more than two distinct inputs".into());
        }
        let conflicts = |id: &String, p: &Pending| {
            supersedes.contains(id) || p.serials.iter().any(|sn| required.contains(sn))
        };
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
            if self.quarantined.as_ref().is_some_and(|p| {
                p.asset == o.asset && p.serials.iter().any(|sn| self.notes.contains_key(sn))
            }) {
                return Err("Retired issued spend is quarantined. Check its original order and confirm this is a different payment.".into());
            }
            for (id, p) in &self.attempts {
                if p.asset == o.asset
                    && !p.conflict_final
                    && p.order_id.is_none()
                    && p.payment.parts()?.0.mode != Mode::Shield
                    && !conflicts(id, p)
                {
                    return Err(format!("Imported spend {id} is unresolved. Retry its bytes, select it for a replacement sharing an input, or confirm this is a different payment.").into());
                }
            }
        }
        if let Some((id, _)) = self.attempts.iter().find(|(id, attempt)| {
            order_id.is_some()
                && attempt.order_id == order_id
                && !attempt.conflict_final
                && !conflicts(id, attempt)
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
        let own = self.next_own_stealth_address()?;
        let fallback_recipient = if o.mode == Mode::Internal {
            None
        } else {
            Some(self.next_own_stealth_address()?)
        };
        let net = &self.point.network;
        let chain = pq::Chain {
            id: &net.chain_id,
            genesis: net.genesis()?,
            protocol: net.protocol_version,
        };
        let recipient = match o.mode {
            Mode::Internal => keys::decode(
                net,
                o.recipient.as_deref().ok_or("missing receive address")?,
            )?,
            _ => fallback_recipient.unwrap(),
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
            let shared = required
                .iter()
                .map(|sn| self.notes.get(sn).ok_or("replacement input is spent"))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            for (sn, n) in self.notes.iter().filter(|(_, n)| n.asset == o.asset) {
                if self
                    .quarantined
                    .as_ref()
                    .is_some_and(|p| p.serials.contains(sn))
                {
                    continue;
                }
                if required.contains(sn) {
                    groups.entry(n.epoch).or_default();
                } else if !self
                    .attempts
                    .iter()
                    .any(|(id, p)| !p.conflict_final && !conflicts(id, p) && p.serials.contains(sn))
                {
                    groups.entry(n.epoch).or_default().push(n);
                }
            }
            for (e, mut notes) in groups {
                if shared.iter().any(|n| n.epoch != e) {
                    continue;
                }
                notes.sort_by_key(|n| std::cmp::Reverse(n.value));
                notes.splice(0..0, shared.iter().copied());
                notes.truncate(2);
                if notes.iter().map(|n| u128::from(n.value)).sum::<u128>() >= u128::from(spend) {
                    if required.len() < 2 && notes[0].value >= spend {
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
            Mode::Shield => {
                if mint <= inner_fee {
                    return Err("mint must exceed network fee".into());
                }
                mint - inner_fee
            }
            Mode::Internal => o.amount,
            Mode::Exit => 0,
        };
        let change = u64::try_from(
            total
                .checked_sub(u128::from(value) + u128::from(exit) + u128::from(inner_fee))
                .ok_or("invalid payment balance")?,
        )?;
        // Randomly shuffle both input slots and output slots so 1-input spends never
        // leak real-vs-dummy slot positions on-chain.
        if OsRng.next_u32() & 1 == 1 {
            inputs.swap(0, 1);
        }
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
        if !required.iter().all(|sn| serials.contains(sn)) {
            return Err("replacement lacks a required shared input".into());
        }
        for (id, pins) in supersedes.iter().zip(parent_inputs) {
            let parent = self.attempts.get_mut(id).expect("validated replacement");
            parent.shared_input = pins.first().cloned();
            parent.other_shared_input = pins.get(1).cloned();
            parent.phase = Phase::Replaced;
        }
        let mut review_terms = json!({"mode":o.mode,"asset":o.asset,"amount":o.amount.to_string(),
            "fee":o.fee.to_string(),"recipient":o.recipient,"payer":o.payer,
            "sequence":o.sequence.map(|n| n.to_string()),"migration":o.migration.is_some()});
        if let Some(id) = &o.replace {
            review_terms["replace"] = json!(id);
        }
        if let Some(id) = &o.also_replace {
            review_terms["also_replace"] = json!(id);
        }
        if o.different_payment {
            review_terms["different_payment"] = json!(true);
        }
        self.latest = Some(hash.clone());
        self.attempts.insert(
            hash.clone(),
            Pending {
                phase: Phase::Prepared,
                conflict_final: false,
                shared_input: required.first().cloned(),
                other_shared_input: required.get(1).cloned(),
                supersedes,
                order_id,
                review_terms: Some(review_terms),
                migration: o.migration,
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

    fn funded(asset: &str, values: [u64; 2]) -> Wallet {
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
                hex::encode(nullifier(&key.r_pk, &coin)),
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
        let mut w = funded(postfiat_types::PQ_REHEARSAL_ISSUE, [5, 5]);
        let first = w
            .prove(serde_json::from_value(exit(2, 1)).unwrap())
            .unwrap();
        let id = first["action_hash"].as_str().unwrap();
        let mut state = serde_json::to_value(&w).unwrap();
        let mut pending = state["attempts"]
            .as_object_mut()
            .unwrap()
            .remove(id)
            .unwrap();
        for k in [
            "order_id",
            "phase",
            "review_terms",
            "migration",
            "shared_input",
            "other_shared_input",
            "supersedes",
            "conflict_final",
        ] {
            pending.as_object_mut().unwrap().remove(k);
        }
        state["pending"] = pending;
        let mut w: Wallet = serde_json::from_value(state).unwrap();
        let mut order = exit(1, 1);
        assert!(w
            .prove(serde_json::from_value(order.clone()).unwrap())
            .unwrap_err()
            .to_string()
            .contains("different payment"));
        let mut pay = order.clone();
        pay["mode"] = json!("Internal");
        pay["recipient"] = json!(w.address(0).unwrap());
        assert!(w
            .prove(serde_json::from_value(pay).unwrap())
            .unwrap_err()
            .to_string()
            .contains("different payment"));

        let mut other = order.clone();
        other["asset"] = json!(pq::NATIVE_ASSET_HEX);
        assert!(w
            .prove(serde_json::from_value(other).unwrap())
            .unwrap_err()
            .to_string()
            .contains("insufficient funds"));
        let mut deposit = order.clone();
        deposit["mode"] = json!("Shield");
        assert!(w
            .prove(serde_json::from_value(deposit).unwrap())
            .unwrap_err()
            .to_string()
            .contains("required public payer"));

        let original = w.prepared(Some(id)).unwrap();
        let mut replacement = exit(2, 1);
        replacement["replace"] = json!(id);
        let replacement = w
            .prove(serde_json::from_value(replacement).unwrap())
            .unwrap();
        let next = &w.attempts[replacement["action_hash"].as_str().unwrap()];
        assert!(next
            .serials
            .contains(w.attempts[id].shared_input.as_ref().unwrap()));
        assert_eq!(w.prepared(Some(id)).unwrap(), original);
        order["different_payment"] = json!(true);
        assert!(w.prove(serde_json::from_value(order).unwrap()).is_ok());
    }

    #[test]
    fn recovery_exit_conflicts_with_both_five_atom_orders() {
        let mut w = funded(postfiat_types::PQ_REHEARSAL_ISSUE, [5, 5]);
        let mut parents = vec![];
        for amount in [1, 2] {
            let proof = w
                .prove(serde_json::from_value(exit(amount, 1)).unwrap())
                .unwrap();
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
        let mut reopened: Wallet = serde_json::from_value(state).unwrap();
        let mut retry = exit(1, 1);
        retry["replace"] = json!(id);
        let next = reopened
            .prove(serde_json::from_value(retry).unwrap())
            .unwrap();
        let next = &reopened.attempts[next["action_hash"].as_str().unwrap()];
        for sn in &w.attempts[id].serials {
            assert!(next.serials.contains(sn));
        }
        for parent in parents {
            assert!(w.attempts[&parent]
                .serials
                .iter()
                .any(|sn| w.notes.contains_key(sn) && w.attempts[id].serials.contains(sn)));
        }
    }

    #[test]
    fn retired_pending_is_quarantined_in_the_encrypted_wallet() {
        use crate::{service::execute, storage::Store};
        use base64::{engine::general_purpose::STANDARD, Engine};
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("wallet");
        std::fs::create_dir(&home).unwrap();
        std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).unwrap();
        let key = tempfile::NamedTempFile::new_in(tmp.path()).unwrap();
        std::fs::write(key.path(), [42; 32]).unwrap();
        let store = Store::new(key.path(), &home).unwrap();
        let native = pq::NATIVE_ASSET_HEX;
        let mut state = serde_json::to_value(funded(native, [5, 5])).unwrap();
        let wire =
            include_bytes!("../../privacy_binius/tests/fixtures/pre-profile-2-issued-exit.bin");
        let inner = &wire[72 + 20 + 8 + pq::PUBLIC_KEY_BYTES + pq::SIGNATURE_BYTES..];
        let retired = json!({"payment":{"body":STANDARD.encode(wire)}, "asset":hex::encode(&wire[7..55]),
                           "serials":[hex::encode(&inner[112..160]),hex::encode(&inner[160..208])],"delta":-2});
        state["pending"] = retired.clone();
        let path = home.join("wallet.enc");
        let tag = b"postfiat/pq-wallet/v1/state";
        store.save(&path, tag, &state, true).unwrap();
        let balance = execute(&home, &store, json!({"op":"balance"})).unwrap();
        assert_eq!(balance["finalized"][native], 10);
        assert!(!balance["quarantined"].is_null());
        let backup = tmp.path().join("backup");
        execute(&home, &store, json!({"op":"backup","backup":backup})).unwrap();
        let recovery: Value = store
            .load(&backup, b"postfiat/pq-wallet/v1/recovery")
            .unwrap();
        assert_eq!(recovery, state["recovery"]);
        let saved: Value = store.load(&path, tag).unwrap();
        assert_eq!(saved["quarantined"], retired);
        assert_eq!(saved["notes"], state["notes"]);
        assert!(execute(&home, &store, json!({"op":"prepared"})).is_err());
        let mut order = exit(4, 1);
        order["asset"] = json!(native);
        execute(&home, &store, json!({"op":"prove","order":order})).unwrap();
        assert!(PqPayment::from_bytes(wire.to_vec()).is_err());
    }

    #[test]
    fn issued_spends_select_notes_for_amount_plus_fee_without_a_signer() {
        let asset = postfiat_types::PQ_REHEARSAL_ISSUE;
        let mut wallet = funded(postfiat_types::PQ_REHEARSAL_ISSUE, [10, 2]);
        for mode in [Mode::Internal, Mode::Exit] {
            let recipient = if mode == Mode::Internal {
                wallet.address(0).unwrap()
            } else {
                format!("pf{}", "33".repeat(20))
            };
            let order = |fee| Order {
                migration: None,
                also_replace: None,
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
                migration: None,
                also_replace: None,
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
