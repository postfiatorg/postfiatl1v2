//! Rebuild a local witness from the birthday frontier and subsequent public leaves.
use pft_binius_shielded::{
    payment::{Opening, PaymentTree},
    shake256::{empty_hashes, merkle_node},
};
use postfiat_types::pq_payment::{PqEpoch, PQ_CAPACITY, PQ_DEPTH};
use serde::{Deserialize, Serialize};
use std::{cell::RefCell, collections::HashMap};

type Node = [u8; 48];
pub(crate) fn digest(s: &str) -> Result<Node, &'static str> {
    hex::decode(s)
        .ok()
        .and_then(|b| b.try_into().ok())
        .ok_or("invalid digest")
}
#[derive(Default, Serialize, Deserialize)]
pub struct History {
    pub start: PqEpoch,
    pub leaves: Vec<String>,
    #[serde(default)]
    pub terminal: Option<String>,
}
impl History {
    pub fn count(&self) -> usize {
        self.start.count as usize + self.leaves.len()
    }
    pub fn tree(&self, root: Node) -> Tree<'_> {
        Tree {
            history: self,
            root,
            empty: empty_hashes(),
            cache: RefCell::new(HashMap::new()),
        }
    }
}

/// Only sibling subtrees for the selected inputs are needed. A depth-first
/// fold uses a depth-24 stack, memoizing internal nodes so 2-input witnesses
/// never recompute shared subtrees.
pub struct Tree<'a> {
    history: &'a History,
    root: Node,
    empty: [Node; 25],
    cache: RefCell<HashMap<(usize, usize), Node>>,
}
impl Tree<'_> {
    fn subtree(&self, level: usize, pos: usize) -> Option<Node> {
        if let Some(&cached) = self.cache.borrow().get(&(level, pos)) {
            return Some(cached);
        }
        let start = pos << level;
        let h = self.history;
        if start >= h.count() {
            return Some(self.empty[level]);
        }
        let birthday = h.start.count as usize;
        if start + (1 << level) <= birthday {
            // For a post-birthday input, each complete left sibling here is
            // exactly one of the aligned subtrees in the birthday frontier.
            if level >= PQ_DEPTH || pos + 1 != birthday >> level {
                return None;
            }
            return digest(h.start.frontier[level].as_ref()?).ok();
        }
        if level == 0 {
            return digest(h.leaves.get(start.checked_sub(birthday)?)?).ok();
        }
        let node = merkle_node(
            &self.subtree(level - 1, pos * 2)?,
            &self.subtree(level - 1, pos * 2 + 1)?,
        );
        self.cache.borrow_mut().insert((level, pos), node);
        Some(node)
    }
}
impl PaymentTree for Tree<'_> {
    fn root(&self) -> Node {
        self.root
    }
    fn opening(&self, index: usize, coin: &Node) -> Option<Opening> {
        if self.subtree(0, index).as_ref() != Some(coin) {
            return None;
        }
        let mut dirs = [0; 24];
        let mut sibs = [[0; 48]; 24];
        for level in 0..PQ_DEPTH {
            let pos = index >> level;
            dirs[level] = (pos & 1) as u8;
            sibs[level] = self.subtree(level, pos ^ 1)?;
        }
        Some((dirs, sibs))
    }
}

/// Advance the verified frontier for a whole pair, retaining only pair roots.
/// Unlike a witness rebuild this costs at most two depth-24 paths per payment.
pub(crate) fn advance(
    state: &mut postfiat_types::pq_payment::PqAsset,
    pair: &postfiat_types::pq_payment::PqPair,
) -> crate::Result<Option<String>> {
    use postfiat_types::pq_payment::PQ_WINDOW;
    let tree = &mut state.writable;
    if state.epoch != pair.epoch || tree.count != pair.first_index || tree.count > PQ_CAPACITY - 2 {
        return Err("unexpected output position".into());
    }
    let mut root = [0; 48];
    for output in &pair.outputs {
        let mut pos = tree.count;
        root = digest(&output.commitment)?;
        for slot in &mut tree.frontier {
            if pos & 1 == 0 {
                *slot = Some(hex::encode(root));
                break;
            }
            root = merkle_node(&digest(&slot.take().ok_or("missing frontier")?)?, &root);
            pos >>= 1;
        }
        tree.count += 1;
    }
    if tree.count != PQ_CAPACITY {
        root = tree.frontier_root()?;
    }
    if tree.anchors.len() == PQ_WINDOW {
        tree.anchors.pop_front();
    }
    tree.anchors.push_back(hex::encode(root));
    if tree.count == PQ_CAPACITY {
        state.frozen_epochs_root = postfiat_crypto_provider::hash_hex(
            "postfiat.pq.frozen_epoch.v1",
            &serde_json::to_vec(&(&state.frozen_epochs_root, &pair.asset, state.epoch, &tree))?,
        );
        state.epoch = state.epoch.checked_add(1).ok_or("epoch overflow")?;
        *tree = PqEpoch::default();
        return Ok(Some(hex::encode(root)));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pft_binius_shielded::shake256::MerkleTree24;
    #[test]
    fn post_birthday_siblings_match_full_tree_for_every_offset() {
        let mut reference = MerkleTree24::new();
        let mut prefix = PqEpoch::default();
        let mut leaves = vec![];
        for i in 0u8..34 {
            let coin = [i + 1; 48];
            reference.append(coin);
            leaves.push(coin);
        }
        for cut in 0..leaves.len() {
            let history = History {
                start: prefix.clone(),
                leaves: leaves[cut..].iter().map(hex::encode).collect(),
                terminal: None,
            };
            let tree = history.tree(reference.root());
            for (index, leaf) in leaves.iter().enumerate().skip(cut) {
                assert_eq!(
                    tree.opening(index, leaf).unwrap(),
                    reference.auth_path(index)
                );
            }
            let mut coin = leaves[cut];
            let mut pos = cut;
            for slot in &mut prefix.frontier {
                if pos & 1 == 0 {
                    *slot = Some(hex::encode(coin));
                    break;
                }
                coin = merkle_node(&digest(&slot.take().unwrap()).unwrap(), &coin);
                pos >>= 1;
            }
            prefix.count += 1;
        }
    }
}
