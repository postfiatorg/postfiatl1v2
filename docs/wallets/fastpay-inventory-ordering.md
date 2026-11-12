# FastPay Inventory Ordering

The web wallet's FastPay inventory has two sources: the gateway's live feed
(`wallet_subscribe`) and the wallet's own all-pages `owned_objects` read. To
let one replace the other safely, the wallet needs to know which answer is
newer. Block height cannot tell it: a FastPay effect changes an owner's objects
without a new block. A gateway clock (`observed_at_ms`) cannot tell it either:
clock skew, a clock rollback, another gateway or a slow, unrelated read in
the same snapshot all reorder answers. This note records what the node has
today, what it would have to expose, and the rule the wallet follows until
then.

## What the node has today

- `view_digest` (`crates/node/src/lifecycle_queries.rs:3237-3258`) identifies
  an owner's object view. It shows that a view is different, not which view
  is newer.
- `height` and `state_root` (`lifecycle_queries.rs:3460-3461`) are the
  finalized tip. They do not change when a FastPay effect does.
- A per-node FastPay apply position exists, but only internally. The view
  `owned_objects` reads is the finalized ledger plus the certified effects in
  the durable speculative journal (`read_fastpay_ledger`,
  `crates/node/src/fastpay_recovery_node.rs:37-114`). Each journal effect
  records the finalized tip it was applied on and
  `application_sequence = fastpay_version_fences.len()`
  (`fastpay_recovery_node.rs:26-33,223-236`). Effects are replayed in that
  order (`:56-67,:89-95`), and the journal is written atomically, so it
  survives a restart (`:169-184`). The pair (finalized height, fence count of
  the view) therefore grows with every FastPay effect on one node and with
  every block. It is not returned by any RPC.

No field available today orders two answers, so the wallet does not rank them.

## What the node would have to expose

1. **Node.** Add `inventory_revision` `{finalized_height, fastpay_sequence}`
   to `OwnedObjectsReport` (`crates/node/src/node_types.rs:3276`). Compute it
   in `owned_objects` under the ordered-commit lock that already covers the
   ledger and tip reads (`lifecycle_queries.rs:3393-3398`). Add the
   answering node's id beside it. This is a read-side change only: no
   consensus rule, state format or signed payload changes. Before shipping,
   prove three properties in node tests: the revision only grows at one
   height, because minority effects are dropped only at a new height; it
   survives a restart; and it either survives a journal snapshot restore
   (`fastpay_recovery_node.rs:186-196`) or forces a new node id. The
   non-transactional storage path (`:38-40`) needs the same proof.
2. **SDK.** Teach the response validators to accept the two fields
   (`crates/rpc_sdk/src/response_validation.rs:6136` and the Python client).
3. **Gateway.** The live feed is a fleet-majority answer. Validators are
   grouped by their canonical result (`wallet-proxy/rpc-routing.js:1574-1597`,
   `readFleetRpcMajority` at `:1606`), and the answer names a set of
   validators, not one node. The gateway would have to leave the revision out
   of the grouping key and return each agreeing validator's revision, plus a
   subscription epoch. The wallet's own pages are forwarded
   (`wallet-proxy/server.js:396`) to an upstream that the answer does not
   name. They would have to be pinned to a named node.
4. **Wallet.** Compare answers only by (node, revision). A lower revision
   from the same node never replaces a higher one. An answer from another
   node, or one received after a reconnect (a new epoch), forces a fresh
   all-pages read. An invalidation stays in force until a view at or after
   its revision is published.

The cost is one node field pair, two validator updates and gateway grouping
work. The node already holds both values, so reads cost no more.

## Interim rule

Implemented in `wallet-web/src/lib/fastpay-inventory.js`:

- A feed answer is never published. One whose view differs from the published
  view, complete or partial, invalidates it. Selection becomes unavailable
  (`loading`) and the wallet reads.
- Only the wallet's own all-pages read is published. It is published only
  when a second read returns the same `view_digest`, that digest is the
  latest feed view (or no feed answer has arrived), and the coordinator has
  not already replaced it. The latest feed view is a standing constraint on
  every later refresh, not only until the first confirmation; replaced
  digests are kept for the session (the last 64). A read of another view
  may have reached a lagging node, or the feed answer may have been delayed.
  The wallet cannot tell which, so it fails closed.
- A refresh that cannot confirm a view in three attempts reports
  `view_unstable`. After that, feed answers only update the feed view, and
  the error stays until a refresh publishes a confirmed read.
- Retry is that refresh plus one resynchronization. When the confirmed view
  differs from the latest feed view, Retry needs a second confirmed read of
  the same view; it then replaces the feed view with it and publishes it.
  A replaced view is never adopted. Retry does not ask the gateway for a new
  feed answer: the subscription has no resend request, and the gateway sends
  only changed snapshots.

Costs and limits: every publication needs two all-pages reads, and a Retry
past a stale feed view four. A plain refresh never moves past the latest
feed view, so a genuinely newer read waits for the feed or for Retry. A
lagging node that agrees with an equally lagging feed still looks current,
and Retry can adopt a lagging node's view that was never published in this
session (for example after the feed named a newer one). Only a node revision
removes those cases. The Send screen keeps FastPay disabled.

Regression tests R1-R5, RB1-N1 (E1, E2, E12 and the compact reproduction)
and RB1-N2 in `wallet-web/src/lib/fastpay-inventory.test.js` reproduce the
second reading's and its re-read's sequences.
