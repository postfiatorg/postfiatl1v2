//! Pure reconstruction of the indexes from authenticated output pairs. Execution
//! and cold import use the same append rules. Only node execution admits proofs.
use super::*;
use imbl::{OrdMap, OrdSet};
use sha3::digest::{ExtendableOutput, Update, XofReader};
use std::{
    collections::VecDeque,
    sync::{Arc, OnceLock},
};

pub const PQ_DEPTH: usize = 24;
pub const PQ_CAPACITY: u32 = 1 << PQ_DEPTH;
pub const PQ_WINDOW: usize = 256;
type Node = [u8; 48];

fn parent(left: &Node, right: &Node) -> Node {
    let mut h = sha3::Shake256::default();
    h.update(&[3]);
    h.update(left);
    h.update(right);
    let mut out = [0; 48];
    h.finalize_xof().read(&mut out);
    out
}

fn empty() -> &'static [Node; PQ_DEPTH + 1] {
    static EMPTY: OnceLock<[Node; PQ_DEPTH + 1]> = OnceLock::new();
    EMPTY.get_or_init(|| {
        let mut hashes = [[0; 48]; PQ_DEPTH + 1];
        for level in 0..PQ_DEPTH {
            hashes[level + 1] = parent(&hashes[level], &hashes[level]);
        }
        hashes
    })
}

fn node(s: &str) -> Result<Node, &'static str> {
    if s.len() != 96
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("pq_noncanonical_digest");
    }
    hex_to_bytes(s)
        .map_err(|_| "pq_digest")?
        .try_into()
        .map_err(|_| "pq_digest")
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PqEpoch {
    pub count: u32,
    pub frontier: [Option<String>; PQ_DEPTH],
    pub anchors: VecDeque<String>,
}

impl Default for PqEpoch {
    fn default() -> Self {
        Self {
            count: 0,
            frontier: Default::default(),
            anchors: [bytes_to_hex(&empty()[PQ_DEPTH])].into(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PqIndexes {
    pub pairs: u64,
    serials: OrdSet<Node>,
    commitments: OrdSet<Node>,
    frozen: OrdMap<(Node, u64), Arc<PqEpoch>>,
    // Completed subtrees never change. Partial right edges at any historical
    // count are obtained by padding these subtrees with the recursive empties.
    nodes: OrdMap<(Node, u64, u8, u32), Node>,
    ordered_pages: OrdMap<u64, String>,
    pages: OrdMap<(Node, u64, u32), String>,
}

impl PqIndexes {
    pub fn is_spent(&self, serial: &str) -> bool {
        node(serial).is_ok_and(|s| self.serials.contains(&s))
    }
    pub fn contains_commitment(&self, commitment: &str) -> bool {
        node(commitment).is_ok_and(|c| self.commitments.contains(&c))
    }
    pub fn page_at(&self, ordinal: u64) -> Option<&str> {
        self.ordered_pages.get(&ordinal).map(String::as_str)
    }
    pub fn page_for_output(&self, asset: &str, epoch: u64, index: u32) -> Option<&str> {
        self.pages
            .get(&(node(asset).ok()?, epoch, index / 2))
            .map(String::as_str)
    }

    fn subtree(&self, asset: Node, epoch: u64, count: u32, level: usize, pos: u32) -> Option<Node> {
        if pos << level >= count {
            return Some(empty()[level]);
        }
        if (pos + 1) << level <= count {
            return self.nodes.get(&(asset, epoch, level as u8, pos)).copied();
        }
        Some(parent(
            &self.subtree(asset, epoch, count, level - 1, pos * 2)?,
            &self.subtree(asset, epoch, count, level - 1, pos * 2 + 1)?,
        ))
    }

    fn append(
        &mut self,
        asset: Node,
        epoch: u64,
        tree: &mut PqEpoch,
        coin: Node,
    ) -> Result<Node, &'static str> {
        let mut pos = tree.count;
        let mut carry = coin;
        for level in 0..=PQ_DEPTH {
            self.nodes.insert((asset, epoch, level as u8, pos), carry);
            if level == PQ_DEPTH {
                break;
            }
            if pos & 1 == 0 {
                tree.frontier[level] = Some(bytes_to_hex(&carry));
                break;
            }
            let left = tree.frontier[level].take().ok_or("pq_missing_frontier")?;
            carry = parent(&node(&left)?, &carry);
            pos >>= 1;
        }
        tree.count += 1;
        Ok(carry)
    }
}

impl PqEpoch {
    /// Reconstruct the padded root from the complete left subtrees of a
    /// writable epoch. Frontier occupancy is uniquely determined by count.
    pub fn frontier_root(&self) -> Result<Node, &'static str> {
        if self.count >= PQ_CAPACITY {
            return Err("pq_tree_capacity");
        }
        let mut root = empty()[0];
        for (level, item) in self.frontier.iter().enumerate() {
            if item.is_some() != ((self.count >> level) & 1 != 0) {
                return Err("pq_frontier_count");
            }
            root = if let Some(left) = item {
                parent(&node(left)?, &root)
            } else {
                parent(&root, &empty()[level])
            };
        }
        Ok(root)
    }
}

impl PqPool {
    pub fn tree(&self, asset: &str, epoch: u64) -> Option<&PqEpoch> {
        let state = self.head.assets.get(asset)?;
        if epoch == state.epoch {
            Some(&state.writable)
        } else {
            self.indexes
                .frozen
                .get(&(node(asset).ok()?, epoch))
                .map(Arc::as_ref)
        }
    }
    pub fn witness(
        &self,
        asset: &str,
        epoch: u64,
        count: u32,
        index: u32,
    ) -> Option<[Node; PQ_DEPTH]> {
        let id = node(asset).ok()?;
        let actual = self.tree(asset, epoch)?.count;
        if count > actual || index >= count || count % 2 != 0 {
            return None;
        }
        let mut path = [[0; 48]; PQ_DEPTH];
        for (level, sibling) in path.iter_mut().enumerate() {
            *sibling = self
                .indexes
                .subtree(id, epoch, count, level, (index >> level) ^ 1)?;
        }
        Some(path)
    }

    // Caller publishes the returned view together with its prepared account
    // writes. Path copying in the ordered maps keeps trial cost logarithmic.
    pub fn append_pair(&self, pair: PqPair) -> Result<Self, &'static str> {
        if self.indexes.pairs != self.head.pairs || pair.previous != self.head.last_page {
            return Err("pq_wrong_parent");
        }
        let asset = node(&pair.asset)?;
        let mut state = self
            .head
            .assets
            .get(&pair.asset)
            .cloned()
            .unwrap_or_default();
        if pair.epoch != state.epoch || pair.first_index != state.writable.count {
            return Err("pq_wrong_output_position");
        }
        let read = if pair.read_epoch == state.epoch {
            &state.writable
        } else {
            self.indexes
                .frozen
                .get(&(asset, pair.read_epoch))
                .ok_or("pq_unknown_epoch")?
                .as_ref()
        };
        if !read.anchors.contains(&pair.anchor) {
            return Err("pq_unknown_anchor");
        }
        if state.writable.count > PQ_CAPACITY - 2 || state.writable.count % 2 != 0 {
            return Err("pq_tree_capacity");
        }
        let serials = [node(&pair.serials[0])?, node(&pair.serials[1])?];
        if serials[0] == serials[1] || serials.iter().any(|s| self.indexes.serials.contains(s)) {
            return Err("pq_spent_serial");
        }
        let coins = [
            node(&pair.outputs[0].commitment)?,
            node(&pair.outputs[1].commitment)?,
        ];
        if coins[0] == coins[1]
            || coins.contains(&[0; 48])
            || coins.iter().any(|c| self.indexes.commitments.contains(c))
        {
            return Err("pq_invalid_commitment");
        }
        for out in &pair.outputs {
            out.envelope()?;
        }
        if pair.height == 0
            || (pair.kind == Mode::Shield) != (pair.mint != 0)
            || (pair.kind == Mode::Exit) != (pair.exit != 0)
            || (asset != [0; 48] && pair.fee != 0)
        {
            return Err("pq_invalid_flow");
        }
        let outflow = pair.exit.checked_add(pair.fee).ok_or("pq_invalid_flow")?;
        state.reserve = (u128::from(state.reserve) + u128::from(pair.mint))
            .checked_sub(u128::from(outflow))
            .and_then(|v| v.try_into().ok())
            .ok_or("pq_reserve")?;
        state.ingress = state
            .ingress
            .checked_add(pair.mint.into())
            .ok_or("pq_ingress")?;
        state.exits = state.exits.checked_add(pair.exit.into()).ok_or("pq_exit")?;
        state.fees = state.fees.checked_add(pair.fee.into()).ok_or("pq_fee")?;
        let mut next = self.clone();
        next.head.pairs = self.head.pairs.checked_add(1).ok_or("pq_pair_count")?;
        for serial in serials {
            next.indexes.serials.insert(serial);
        }
        for coin in coins {
            next.indexes.commitments.insert(coin);
        }
        let tree = &mut state.writable;
        next.indexes.append(asset, state.epoch, tree, coins[0])?;
        let carry = next.indexes.append(asset, state.epoch, tree, coins[1])?;
        let root = if tree.count == PQ_CAPACITY {
            carry
        } else {
            tree.frontier_root()?
        };
        if tree.anchors.len() == PQ_WINDOW {
            tree.anchors.pop_front();
        }
        tree.anchors.push_back(bytes_to_hex(&root));
        if tree.count == PQ_CAPACITY {
            state.frozen_epochs_root = digest_record(
                "postfiat.pq.frozen_epoch.v1",
                &(&state.frozen_epochs_root, &pair.asset, state.epoch, &tree),
            );
            next.indexes
                .frozen
                .insert((asset, state.epoch), Arc::new(std::mem::take(tree)));
            state.epoch = state.epoch.checked_add(1).ok_or("pq_epoch_overflow")?;
        }
        // These append commitments authenticate the deterministic index insertions,
        // including the location of both wire-ordered records in the page.
        let page = pair.digest();
        let head = &mut next.head;
        head.serial_index_root = digest_record(
            "postfiat.pq.serial_index.v1",
            &(&head.serial_index_root, &pair.serials),
        );
        head.commitment_index_root = digest_record(
            "postfiat.pq.commitment_index.v1",
            &(
                &head.commitment_index_root,
                &pair.asset,
                pair.epoch,
                pair.first_index,
                &pair.outputs[0].commitment,
                &pair.outputs[1].commitment,
            ),
        );
        head.output_index_root = digest_record(
            "postfiat.pq.output_index.v1",
            &(
                &head.output_index_root,
                &pair.asset,
                pair.epoch,
                pair.first_index,
                &page,
            ),
        );
        head.last_page = page.clone();
        next.indexes.ordered_pages.insert(self.head.pairs, page.clone());
        next.indexes
            .pages
            .insert((asset, pair.epoch, pair.first_index / 2), page);
        next.indexes.pairs = head.pairs;
        head.assets.insert(pair.asset.clone(), state);
        next.pending.push_back(Arc::new(pair));
        Ok(next)
    }
}
