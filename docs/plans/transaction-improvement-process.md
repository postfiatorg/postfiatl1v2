# Transaction Improvement Process

Status: active sprint specification
Created: 2026-06-28
Scope: PostFiat L1 transaction correctness, finality, wallet/RPC parity, FastPay, and latency optimization

## Purpose

The current transaction stack is not fast or reliable enough to keep treating
demo transactions as product-ready behavior. A browser wallet send produced a
pending transaction instead of finality, FastPay failed immediately in the UX,
the Python helper path did not represent the browser wallet path, validator
endpoints diverged, and recovery devolved into manual binary replacement and
validator catch-up. This process exists to replace that failure mode with a
measured, repeatable, network-level transaction system.

This is not a StakeHub sprint. This is PFTL / `postfiatl1v2` work. StakeHub and
the web wallet are consumers of the L1 transaction surface; they do not define
whether the network works.

The objective is optimization. A slow transaction path, broken RPC path,
stalled validator, failed FastPay route, mismatched Python helper, or wallet
send that does not reach a receipt is an engineering defect to improve. The
agent must not stop at reframing expectations or explaining that a component is
bad when the component is inside this repo or controlled devnet. The required
behavior is: measure the failure, identify the bottleneck,
change the code, tooling, configuration, or fleet state, and rerun the gate.
If the failing component is an RPC server, wallet proxy, validator service,
Python helper, wallet UI, deployment script, or controlled devnet host, the
default action is to fix or replace that component. A red RPC gate is a work
queue item, not a stopping condition. The acceptable loop is optimize, deploy,
measure, and repeat until the gate passes or a specific missing key, machine,
or external dependency prevents execution.

The overnight job should run continuously until every stage below is either
completed with evidence or explicitly blocked by a concrete missing credential,
machine, or code dependency. Resetting the controlled WAN devnet is authorized
if it is the cleanest way to restore a known-good network state.

## Non-Negotiable Outcomes

- A browser wallet transaction must reach a final receipt through the same
  production RPC path advertised by `server_info`.
- Python transaction helpers must exercise the same WAN semantics as the
  wallet, not a local `apply-batch` harness path masquerading as end-to-end.
- FastPay must work end-to-end: wrap, balance, send, unwrap, finality, and
  receipt.
- Latency targets in docs or blog posts must become optimization targets backed
  by current run evidence. If the measured WAN path misses the target, the
  system must be improved until the target is met or the target is explicitly
  replaced by a new engineering gate.
- Controlled RPC/network latency is owned by this sprint. The next action after
  a slow quote, slow submit, route miss, stale mempool, or stalled validator is
  an implementation, configuration, fleet, or topology improvement followed by
  a measured rerun.
- Validator liveness must be proven before every transaction acceptance run:
  height, root, binary hash, service command line, RPC capability, quorum, and
  mempool state.
- Transaction permutations must be covered beyond simple PFT transfers:
  trustline setup, issued asset transfer, offer/atomic settlement, FastPay
  object movement, Orchard ingress/spend/withdraw, Asset-Orchard actions, and
  bridge-related batches where applicable.
- No more "it worked in Python" acceptance unless the exact path, RPC endpoint,
  finality mechanism, receipt, block height, state root, and validator quorum
  are recorded.
- Every slow, flaky, stalled, or divergent transaction path is tracked as a
  system improvement item. The next action is code, RPC, proxy, fleet, tooling,
  or wallet optimization, followed by a rerun of the measured gate.

## Incident Baseline

The specific failure chain that triggered this process:

- Browser wallet submitted transaction
  `3dfa17aff192dbe8d46ee0a57b78c0874e8a87d0a0800d84f54c116876b09f0defa1de675bc01797c4b5c227db0f9022`
  and showed pending finality instead of a final receipt.
- The transaction was a memo-bearing `payment_v2` from
  `pfde0ba09f38b1748f8d77709715e1095a0ff74d0f` to
  `pf17c682e5b1c913527527635df78cceb1d7d80fa2`, amount `5 PFT`.
- Validator-1 held the transaction in mempool at height `477` with no receipt.
- Validator-4 finalized the same signed transaction at height `478` after
  manual binary replacement and direct RPC submission.
- The finality round took about `764 ms`, with block hash
  `9f5a02ce2ee8426f2163b19aa468645d5acf588f33723d37ca7b59c800ded3f03a8c8e123d3f89c14dd87ea8133b4fa8`,
  state root
  `723a3afdbe1cb1b62238e1b9b25559ab796fabf25b170deb310775199a35e502277292f8c3267c26ca0480928c40cd87`,
  and receipt code `accepted`.
- Validator-1 then remained stale at height `477` while most peers reached
  height `478`.
- Re-sending the certified block to validator-1 failed with
  `external block certificate proposal hash mismatch`.
- Recovery started drifting into long validator catch-up / force-sync work,
  which is unacceptable as a normal transaction path.
- The wallet proxy and remote RPC capabilities were inconsistent with observed
  behavior. The system advertised finality, but the active binary/service path
  did not reliably support the wallet's `payment_v2` finality method.
- The Python helper surface had previously been treated as proof of
  transactions, but that path did not accurately represent the browser wallet
  WAN flow.
- Earlier helper failures around validator/proposer keys showed another
  mismatch: local harness paths assume access to all validator data dirs or
  the relevant proposer key, while the WAN devnet has separate validator keys
  on separate machines.

## Diagnosis Themes

### 1. Local Harness Semantics Leaked Into WAN Performance Work

`apply-batch` and local multi-validator helper functions are useful for local
development. They are not proof that the WAN devnet can accept, certify, and
serve a wallet transaction. The WAN path must submit to RPC, route finality to
the right proposer, collect votes from validators, commit a block, propagate
the certificate, and make the receipt visible through public RPC.

Completion requires deleting or quarantining any docs, tests, or helper names
that imply local harness success equals WAN finality.

### 2. RPC Capability Discovery Was Not Binding

The wallet trusted RPC capability metadata, but observed behavior showed
method rejection and endpoint-specific divergence. `server_info.rpc` must be a
contract with the active process, not a stale assumption or proxy injection.

Completion requires a client-visible health report that proves the exact
method to be used is accepted by the endpoint before the send button is
enabled.

### 3. Proposer-Affinity Was Hidden From Users And Tools

The finality RPC can require local proposer status for the next block. A
wallet-facing endpoint that is not the current proposer can fail or stall
unless the proxy routes to the current proposer or the network supports a
proper submit-and-forward path.

Completion requires one of:

- a proposer-aware transaction router;
- a mempool gossip path that lets any RPC accept and forward safely;
- an RPC finality service that does not require the contacted node to be the
  current proposer; or
- a clear temporary devnet rule that the wallet proxy always targets the
  current proposer, with automated rotation.

### 4. Validator Fleet State Was Not Treated As A Release Gate

The fleet was allowed to continue serving reads and wallet endpoints while
validators were on different binaries or states. This produced stale mempools,
proposal hash mismatches, and manual recovery.

Completion requires fleet health to be a precondition for transaction demos,
wallet sends, Python helper runs, and latency measurements.

### 5. FastPay Was Not End-To-End Ready

The FastPay lane needs object lookup, balance display, recipient key
resolution, wrap, unwrap, object spend, finality, and receipt polling. A UI
that renders FastPay controls but immediately fails `owned_objects` or
recipient resolution is not a working FastPay implementation.

Completion requires independent FastPay tests at the RPC, Python, and wallet
layers.

### 6. Latency Targets Exceed Current Performance

A blog or docs latency target such as sub-second or ~1.5 second finality must
be treated as an engineering target for the exact path being measured. A
manually recovered transaction that requires minutes of validator repair means
the transaction path needs improvement, even if the final certified round itself
was fast.

Completion requires current measurement artifacts for cold start, warm send,
finality, receipt availability, validator propagation, and p95/p99 behavior.

## Authorized Remediation Policy

This is a controlled pre-testnet. The overnight process may:

- reset the WAN devnet if the current state is too inconsistent to repair
  cleanly;
- redeploy validator binaries across all six validators;
- restart validator and RPC services;
- rebuild or rotate devnet-only validator keys if the existing deployment
  cannot support proposer rotation and finality;
- clear devnet mempools during a reset;
- regenerate topology files and service units;
- restart wallet proxy services;
- run destructive devnet-only state recovery after preserving a snapshot;
- mark prior devnet transaction evidence as superseded when it is no longer
  representative.

The overnight process must not:

- spend Ethereum mainnet funds;
- touch production user funds;
- assert public decentralization readiness from a project-controlled devnet;
- use local `apply-batch` as a substitute for WAN finality;
- mask failed validator states behind a green UI;
- leave latency targets unmet without an active repair item for the current
  measured path.

## Execution Log

### 2026-06-28T02:21Z - 2026-06-28T02:33Z Fleet Repair And Wallet-Proxy Routing

Status: partial execution, not full sprint completion. Stages 1, 2, and 4
now have live controlled-devnet evidence. The broader transaction permutation,
FastPay, Orchard, and latency gates remain open.

Evidence:

- Initial strict preflight:
  `reports/transaction-improvement/20260628T022120Z/fleet-baseline.json`
- Repair decision, binary deployment log, and state-sync log:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/`
- Final strict preflight after routed wallet-proxy sends:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/post-proxy-finality-preflight.json`
- Direct proposer failure from the old wallet-facing endpoint behavior:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/live-finality-tests/native-validator0-finality.json`
- Direct proposer success against validator-5:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/live-finality-tests/native-validator5-finality.json`
- Wallet-proxy proposer-routed sends:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/proxy-finality/response-1.json`
  and
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/proxy-finality/response-2.json`
- Six-send proposer-rotation proof:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/proxy-finality/response-1.json`
  through
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/proxy-finality/response-6.json`
- Current-source binary deployment:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/current-source-binary-20260628T023832Z.sha256`
  and
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/current-source-binary-deploy-20260628T023832Z.log`
- Memo `payment_v2` finality proof through the wallet proxy:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/proxy-payment-v2-current-source/response.json`
- Final strict preflight:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/post-payment-v2-finality-preflight.json`

Actions completed:

- Added read-only fleet gate command:
  `scripts/wan-devnet-transaction-preflight`.
- Initial preflight was red: validator-1 was one block stale at height `477`,
  validators 1 and 4 had binary hash `345c9dd...`, validators 0/2/3/5 had
  binary hash `aae909...`, and validators 1 and 3 had stale mempool entries.
- Converged validators 1 and 4 to the majority live binary
  `aae90939792dbed11bcc849454d01b967bc5b4875f22e07be85cf0b9e4447428`.
- Repaired validator-1 and validator-3 state from validator-0 with
  `scripts/wan-devnet-state-sync`; remote backups were created at
  `/var/lib/postfiat/validator-1-backup-20260628T022357Z` and
  `/var/lib/postfiat/validator-3-backup-20260628T022419Z`.
- Verified strict preflight green at height `478`, then proved a direct native
  PFT finality submit succeeds when sent to the deterministic proposer
  validator-5: height `479`, receipt `accepted`, total finality time about
  `1076.855 ms`, wall time about `1182.540 ms`.
- Added proposer-aware finality routing to `wallet-proxy/server.js` for
  `mempool_submit_signed_transfer_finality` and
  `mempool_submit_signed_payment_v2_finality`.
- Restarted the wallet proxy on port `8080` with routing enabled.
- Proved two WebSocket wallet-proxy native finality sends:
  height `480` routed to validator-0, total finality time about `988.782 ms`;
  height `481` routed to validator-1, total finality time about `806.846 ms`.
- Extended the proxy proof to six consecutive native finality sends across all
  six proposers:
  height `480` validator-0, height `481` validator-1, height `482`
  validator-2, height `483` validator-3, height `484` validator-4, and
  height `485` validator-5.
- Discovered that the majority live binary `aae909...` still rejected
  `mempool_submit_signed_payment_v2_finality` as not enabled. The local
  current-source node test
  `rpc_cli_tests::mempool_submit_signed_payment_v2_finality_allowed_under_finality_flag`
  passed, so the fleet was redeployed from local release binary
  `9e81989448290c7d6fa7c002477da932b820e6c85ed1609e419ca6519f63f3b9`.
- Proved memo-bearing `payment_v2` finality through the wallet proxy:
  height `486` routed to validator-0, receipt accepted, total finality time
  about `1008.271 ms`, wall time about `2703 ms`.
- Verified final strict preflight green: all six validators at height `486`,
  identical state root
  `a94f8f5bc3a25de58dd9d37d4a6a1dea151c06d3d2b3a725e875fcbd857e1bde1c1b2ebedac52459e3bd4c7d03c47d76`,
  empty mempools.

Checks run:

- `PYTHONPATH=python python3 -m pytest python/tests/test_wan_preflight.py python/tests/test_wallet.py -q`
  - `53 passed`
- `node --check wallet-proxy/server.js`
- `node wallet-proxy/test_proposer_routing.js`
- `npm test` in `wallet-web/`
  - `21 passed`
- `cargo fmt --check`
- `cargo check -p postfiat-rpc-sdk`
- `cargo test -p postfiat-node --all-targets mempool_submit_signed_payment_v2_finality_allowed_under_finality_flag`
- `cargo build --release -p postfiat-node`

Residual risk:

- The fleet is converged on the current local release binary `9e819...`, but
  the source tree is still dirty. A clean release commit/tag and redeploy
  record remain a Stage 10 release-process item.
- Native PFT and memo `payment_v2` finality are proven through the wallet
  proxy. FastPay wrap/send/unwrap, trustline/asset, offers, Orchard,
  Asset-Orchard, and bridge permutations are not yet proven by this execution
  log.
- The six-send routing proof is a functional Stage 4 pass for the current
  controlled fleet. It is not a public decentralization or adversarial network
  result.

### 2026-06-28T02:49Z - 2026-06-28T03:35Z FastPay Controlled-Devnet Repair

Status: Stage 5 Python parity and Stage 6 FastPay now have controlled-devnet
evidence for the wallet-facing proxy path. This does not yet prove that
FastPay is consensus-native block finality: the current bridge mutations are
all-validator proxy broadcasts over project-controlled validator RPCs, while
the block height and canonical account state root remain unchanged.

Evidence:

- FastPay broadcast binary deployment:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/fastpay-broadcast-binary-20260628T024929Z.sha256`
  and
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/fastpay-broadcast-binary-deploy-20260628T024929Z.log`
- JSON payload cap fix deployment:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/fastpay-json-cap-binary-20260628T025636Z.sha256`
  and
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/fastpay-json-cap-binary-deploy-20260628T025636Z.log`
- Corrected 5-of-6 FastPay quorum deployment:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/fastpay-quorum-binary-20260628T030604Z.sha256`
  and
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/fastpay-quorum-binary-deploy-20260628T030604Z.log`
- Failed first FastPay run proving the old 4096-byte `order_json` cap:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/fastpay-live/fastpay-wrap-send-unwrap-20260628T025322Z.json`
- Existing-object send blocked by a slow validator when the client waited for
  all six votes instead of quorum:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/fastpay-live/fastpay-send-unwrap-existing-20260628T025852Z.json`
- First apply/unwrap success with a 5-vote certificate, before the node-side
  quorum text was fixed:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/fastpay-live/fastpay-apply-quorum-unwrap-20260628T030105Z.json`
- Corrected node-side quorum apply with a transient unwrap timeout:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/fastpay-live/fastpay-quorum5-send-unwrap-20260628T030821Z.json`
- Unwrap timeout recovery and validator-wide post-state:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/fastpay-live/fastpay-retry-unwrap-20260628T0319Z.json`
  and
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/fastpay-live/fastpay-post-retry-unwrap-state-20260628T0319Z.json`
- Fresh proxy-backed FastPay cycle:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/fastpay-live/fastpay-fresh-proxy-cycle-20260628T0323Z.json`
- Python helper parity through the WebSocket wallet proxy:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/fastpay-live/python-websocket-fastpay-cycle-20260628T0330Z.json`
- Validator-wide post-Python state:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/fastpay-live/python-fastpay-post-state-20260628T0330Z.json`
- Final strict preflight after FastPay work:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/post-python-fastpay-preflight.json`

Actions completed:

- Added validator-id routing for `owned_sign` in `wallet-proxy/server.js` so
  each FastPay vote request reaches the intended validator instead of the
  proxy's primary validator.
- Added controlled-devnet all-validator broadcasts in `wallet-proxy/server.js`
  for `wrap_owned`, `unwrap_owned`, and `owned_apply`. Broadcast mutations
  require all six validators to be reachable and ledger-converged before the
  mutation is sent.
- Added proxy-generated shared object ids for `wrap_owned` so all validators
  mint the same FastPay object id during a broadcast wrap.
- Added a direct `owned_apply` branch to `rpc-serve`; the method had been in
  the allowlist but fell through to child RPC handling and failed as unknown.
- Raised RPC validation limits for FastPay `order_json` and `cert_json` to
  64 KiB, while retaining tighter defaults for ordinary string parameters.
- Fixed node-side FastPay certificate quorum from `3 of 6` to the same
  Byzantine quorum threshold used elsewhere: `floor(2n/3)+1`, so 6 validators
  require 5 votes.
- Updated the browser transaction builder and Python helper vote collection to
  stop at 5-of-6 quorum rather than waiting for a slow sixth validator.
- Added `PostFiatWebSocketRpcClient` so Python helpers can use the same
  wallet-facing WebSocket proxy path as the browser wallet.
- Added a FastPay capability guard in Python helpers: `wrap_fastpay`,
  `send_fastpay`, and `unwrap_fastpay` now reject raw single-validator RPC
  endpoints unless `server_capabilities()` advertises
  `fastpay_bridge_mode=proxy_broadcast_devnet`.
- Proved a fresh proxy-backed FastPay cycle: wallet A wrapped `2 PFT`, sent
  `1 PFT` to wallet B with 5 validator votes, broadcast-applied on 6/6
  validators with node-side `quorum 5 of 6`, and wallet B unwrapped the object
  to account balance. Wallet B balance moved from `2.000740 PFT` to
  `3.000740 PFT`.
- Proved the same class of cycle through Python helpers using
  `PostFiatWebSocketRpcClient('ws://127.0.0.1:8080')`: wallet B balance moved
  from `3.000740 PFT` to `4.000740 PFT`; post-state shows all six validators
  with wallet B account balance `4.000740 PFT` and zero wallet B FastPay
  objects.

Checks run:

- `PYTHONPATH=python python3 -m pytest python/tests/test_wallet.py python/tests/test_wan_preflight.py -q`
  - `54 passed`
- `python3 -m py_compile python/postfiat_rpc/client.py python/postfiat_rpc/wallet.py`
- `cargo test -p postfiat-rpc-sdk --lib`
  - `25 passed`
- `npm test` in `wallet-web/`
  - `23 passed`
- `node --check wallet-proxy/server.js`
- `node wallet-proxy/test_proposer_routing.js`
- `cargo fmt --check`
- `cargo check -p postfiat-node`
- `scripts/wan-devnet-transaction-preflight --strict-exit --output reports/transaction-improvement/20260628T022227Z-fleet-repair/post-python-fastpay-preflight.json`
  - `GREEN`, 6/6 reachable, 6/6 same height/root, mempool 0.

Residual risk:

- The current FastPay bridge path is a controlled-devnet wallet-proxy broadcast,
  not a consensus-native block transaction. It is acceptable evidence for the
  current wallet demo path, but it must not be described as public-network
  finality.
- `wrap_owned` and `unwrap_owned` still mutate account/owned state outside a
  certified block. The next protocol-quality step is to move these lane
  transitions into a canonical certified transaction or batch type so the
  normal block root changes and receipt path prove them.
- One corrected-quorum run hit a transient `unwrap_owned timeout`. A retry
  succeeded in `1.587s`, all six validators remained converged, and no partial
  unwrap occurred. The wallet/proxy path still needs a sharper typed timeout
  response if a future validator RPC call hangs.
- Stage 7 transaction permutation coverage, Stage 8 latency distribution
  measurement, and Orchard/Asset-Orchard/bridge coverage remain open.

### 2026-06-28T13:49Z - 2026-06-28T14:42Z Transaction Latency Optimization

Status: active optimization pass, not sprint completion. Account-lane finality
and FastPay latency both improved, but the full matrix and wallet Playwright
gate remain open.

Evidence:

- Non-deferred finality rollout:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/rpc-nondeferred-finality-binary-deploy-20260628T134932Z.log`
- Non-deferred finality self-send fix rollout:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/rpc-nondeferred-finality-no-selfsend-binary-deploy-20260628T135629Z.log`
- Direct transfer quote rollout:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/rpc-direct-transfer-quote-binary-deploy-20260628T140521Z.log`
- Structured FastPay `owned_apply` rollout:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/fastpay-structured-owned-apply-binary-deploy-20260628T143601Z.log`
- Post-rollout strict preflight:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/post-fastpay-structured-owned-apply-preflight-20260628T143739Z.json`
- Account-lane baseline with route retries:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/stage8-post-proxy-route-retry-account-full-20260628T132528Z/latency-summary.json`
- Account-lane after route cache and finality fixes:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/stage8-post-finality-route-cache-sample-20260628T140115Z/latency-summary.json`
- FastPay baseline:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/stage8-fastpay-smoke-20260628T140932Z/latency-summary.json`
- FastPay after structured outputs, quorum-fast broadcast, and 10s route cache:
  `reports/transaction-improvement/20260628T022227Z-fleet-repair/stage8-fastpay-status-cache10s-smoke-20260628T144147Z/latency-summary.json`

Actions completed:

- Preserved proxy route metadata in Python finality-submit responses and
  latency reports.
- Routed sequenced account quote reads to the next proposer and primed the
  next proposer route after successful finality.
- Changed RPC finality submit to apply locally during the round and fixed
  certified-send target selection so the proposer does not wait on its own
  public endpoint after local apply.
- Added direct in-process `transfer_fee_quote` handling in `rpc-serve`; the
  measurement showed quote latency mostly comes from ledger/state/network read
  cost, not child-process spawn.
- Parallelized FastPay vote collection in Python and added per-substep timing
  for wrap, sign, vote, apply, and unwrap.
- Made FastPay proxy broadcast return at quorum by default and continue late
  validators best-effort.
- Added structured `OwnedApplyReport` from node RPC so FastPay send returns
  created object ids; the runner now unwraps the recipient output without a
  slow `owned_objects` lookup.
- Extended the FastPay fleet-status cache to cover one wrap/send/unwrap cycle,
  removing repeated six-validator status sweeps from the hot path.

Measured improvement:

- Account lane: native/payment samples moved from multi-second tails caused by
  route retries to warm p50 around `2.9s` and p95/max around `3.9s` in the
  routed-cache samples.
- FastPay full wrap/send/unwrap cycle improved from about `20.0s` p50 in
  `stage8-fastpay-smoke-20260628T140932Z` to `3.297s` p50 in
  `stage8-fastpay-status-cache10s-smoke-20260628T144147Z`.
- Current warm FastPay split in the best sample: wrap about `0.808s`,
  send/apply about `1.678s`, unwrap about `0.811s`.

Checks run:

- `cargo fmt --check`
- `cargo test -p postfiat-node owned_apply -- --nocapture`
- `cargo check -p postfiat-node`
- `PYTHONPATH=python python3 -m pytest python/tests/test_latency.py python/tests/test_wallet.py python/tests/test_wan_preflight.py -q`
  - `65 passed`
- `node --check wallet-proxy/server.js`
- `node wallet-proxy/test_proposer_routing.js`
- `scripts/wan-devnet-transaction-preflight --strict`
  - `GREEN`, 6/6 reachable, height `860`, root `3804d13427591f07234037b9e670309b5a7a1a9e8af4cc153ff0508637ecf38b826000f8c28823651634454e71172c69`.

Remaining improvement work:

- Account-lane p50 is still above the target implied by the latency article.
  The next bottlenecks are quote/state read time and certified-round transport.
- FastPay is now usable but still has a 3s-class full wrap/send/unwrap cycle.
  Warm send-only is faster than the full bridge cycle; the report needs a
  separate FastPay send-only distribution so wallet UX and performance gates
  measure the right path.
- Trustline, asset, offer/atomic, Orchard, Asset-Orchard, and bridge-related
  transaction permutations still need current WAN evidence.

### 2026-06-28T16:00Z - 2026-06-28T16:22Z Wallet Proxy Routing And Deployed RPC Propagation Pass

Status: incremental Stage 8 improvement. The WAN fleet is on a clean release
binary from commit `9edc59c3`, the wallet proxy now defaults account quote
reads to the first validator that has the required parent state, and the proxy
caches the finality responder as a known-good parent-state read endpoint.
This pass improved the reliable account-lane smoke, but the full Stage 8 gate
is still open because next-proposer readiness waits remain in the tail.

Evidence:

- Pre-deploy strict preflight:
  `reports/transaction-improvement/20260628T160048Z-current-preflight.json`
- Release binary rollout:
  `reports/transaction-improvement/20260628T160230Z-inprocess-deferred-deploy/`
- Post-deploy strict preflight with SSH inventory:
  `reports/transaction-improvement/20260628T160400Z-post-inprocess-deploy-preflight.json`
- Post-deploy account smoke before quote-read changes:
  `reports/transaction-improvement/20260628T160448Z-inprocess-deferred-account-smoke/latency-summary.md`
- First-ready quote default smoke:
  `reports/transaction-improvement/20260628T160756Z-first-ready-default-account-smoke/latency-summary.md`
- Final reliable smoke for this pass:
  `reports/transaction-improvement/20260628T162153Z-reliable-first-ready-read-cache-account-smoke/latency-summary.md`

Actions completed:

- Built and deployed `target/release/postfiat-node` hash
  `4776b34ab0cfb913c1c139c2b84c0a0e6d05aaa0ca154115870d5976816870ca`
  to all six validators.
- Verified all six validator and RPC services active with the same binary hash,
  height `980`, identical state root, and empty mempools before mutable tests.
- Restarted the wallet-facing proxy on port `8080`.
- Changed wallet-proxy sequenced account reads from opt-in first-ready routing
  to first-ready by default, with `ENABLE_FIRST_READY_SEQUENCED_READ=false` as
  the rollback switch.
- Added a finality-responder read cache so a quote after a finalized block can
  read from the endpoint that just returned finality for that parent state
  instead of starting with a fresh fleet status sweep.
- Kept optimistic cached finality routing disabled by default after live
  testing showed it removes route waits but can route too early and produce a
  timeout. It remains available only behind
  `OPTIMISTIC_CACHED_FINALITY_ROUTE=true` for controlled experiments.

Measured result:

- Stable final smoke completed `8/8` account-lane sends with no failures.
- Native PFT p50 improved to `1880.166 ms` in the final smoke, with quote p50
  about `672.001 ms`, submit/finality p50 about `1255.517 ms`, and node
  finality p50 about `985.737 ms`.
- Memo `payment_v2` p50 improved to `2305.251 ms` in the final smoke, with
  quote p50 about `292.717 ms`, submit/finality p50 about `2494.627 ms`, and
  node finality p50 about `995.904 ms`.
- Route waits are mostly eliminated on warm submissions but still produce
  `~1.6s` tails when the deterministic next proposer has not yet observed the
  previous certified block.

Checks run:

- `node --check wallet-proxy/server.js`
- `node wallet-proxy/test_proposer_routing.js`
- `python3 -m py_compile python/postfiat_rpc/client.py`
- `PYTHONPATH=python python3 -m pytest python/tests/test_latency.py python/tests/test_wallet.py -q`
  - `62 passed`

Next improvement item:

- Remove the remaining next-proposer readiness tail without using unsafe
  optimistic routing. The likely implementation path is to prioritize certified
  block propagation to the deterministic next proposer inside the RPC finality
  round, or replace status-probe readiness with a typed fast readiness signal
  that is cheaper than a full status RPC.

### 2026-06-28T17:00Z - 2026-06-28T17:25Z Quote Routing And Python Wallet Parity Pass

Status: incremental Stage 5 and Stage 8 improvement. This pass cleaned up the
wallet/Python parity path, removed a measured quote-read bottleneck from the
normal wallet route, and kept the experimental upstream TCP keep-alive behind
an opt-in flag because live A/B measurements showed worse tails.

Evidence:

- Upstream TCP keep-alive A/B sample:
  `reports/transaction-improvement/20260628T170704Z-upstream-keepalive-account-smoke/latency-summary.md`
- Persistent WebSocket plus upstream keep-alive sample:
  `reports/transaction-improvement/20260628T170959Z-persistent-ws-upstream-keepalive-smoke/latency-summary.md`
- Persistent WebSocket without upstream keep-alive sample:
  `reports/transaction-improvement/20260628T171120Z-persistent-ws-no-upstream-keepalive-smoke/latency-summary.md`
- First-ready quote-read sample before the preferred parent-wait route:
  `reports/transaction-improvement/20260628T171519Z-first-ready-no-responder-cache-smoke/latency-summary.md`
- Pre-deploy strict preflight:
  `reports/transaction-improvement/20260628T172155Z-pre-quote-parent-wait-deploy.json`
- Quote parent-wait binary rollout:
  `reports/transaction-improvement/20260628T172233Z-quote-parent-wait-deploy/`
- Post-deploy strict preflight:
  `reports/transaction-improvement/20260628T172346Z-post-quote-parent-wait-deploy.json`
- Preferred parent-wait quote route smoke:
  `reports/transaction-improvement/20260628T172433Z-quote-parent-wait-preferred-smoke/latency-summary.md`
- Post-merge strict preflight:
  `reports/transaction-improvement/20260628T173108Z-post-merge-preflight.json`

Actions completed:

- Changed `PostFiatWebSocketRpcClient` to keep one WebSocket open across
  requests, matching the browser wallet more closely than the prior one-request
  connection model.
- Added an opt-in persistent upstream TCP path in `wallet-proxy/server.js` and
  a regression test proving connection reuse when
  `ENABLE_UPSTREAM_KEEPALIVE=true`.
- Kept upstream TCP keep-alive disabled by default after the A/B sample showed
  `payment_v2` p90/max around `5.96s`, worse than the normal one-shot upstream
  path.
- Added node-side proxy parent-wait support for quote methods:
  `transfer_fee_quote`, `asset_fee_quote`, `escrow_fee_quote`,
  `nft_fee_quote`, and `offer_fee_quote`.
- Changed wallet-proxy sequenced account reads to prefer the low-latency
  validators `validator-2` and `validator-5` and annotate quote RPCs with the
  required parent height/root, letting the selected node wait locally for the
  exact parent state instead of doing a foreground fleet status sweep.
- Disabled the finality-responder read cache by default. Live RTT checks from
  the proxy host showed validator-2 and validator-5 around `10-16ms`, while
  validators 1 and 4 were around `376-381ms` and validators 0 and 3 were around
  `520ms`; caching the last responder could pin reads to a slow endpoint.
- Built and deployed `target/release/postfiat-node` hash
  `45a39cec5b024bd57e155522768bea6801aca18d40d383bd658ee4194d99561f` to all
  six validators.
- Restarted the wallet-facing proxy on port `8080` with upstream keep-alive
  disabled, finality-responder read cache disabled, sequenced read parent-wait
  enabled, and preferred sequenced read validators `validator-2,validator-5`.

Measured result:

- Native quote p50 improved from about `957.859 ms` in
  `20260628T171519Z-first-ready-no-responder-cache-smoke` to about
  `390.009 ms` in `20260628T172433Z-quote-parent-wait-preferred-smoke`.
- Memo `payment_v2` quote p50 improved from about `1121.854 ms` to about
  `351.552 ms`.
- Native account-lane p50 improved from `2419.668 ms` to `2011.324 ms`.
- Memo `payment_v2` p50 improved from `2457.985 ms` to `2042.727 ms`.
- The remaining account-lane tail is now mostly submit/finality propagation:
  `submit_finality_ms` p50 remains about `1.48-1.51s`, with max samples up to
  about `2.86s`.

Checks run:

- `cargo fmt --check`
- `cargo check -p postfiat-node`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test -p postfiat-node --all-targets rpc_proxy_parent_wait_is_limited_to_quote_methods`
- `mkdocs build --strict`
- `node --check wallet-proxy/server.js`
- `node wallet-proxy/test_proposer_routing.js`
- `PYTHONPATH=python python3 -m pytest python/tests/test_latency.py python/tests/test_wallet.py python/tests/test_wan_preflight.py -q`
  - `65 passed`
- `PYTHONPATH=python scripts/wan-devnet-transaction-preflight --ssh-inventory --ssh-user root --output reports/transaction-improvement/20260628T173108Z-post-merge-preflight.json --strict-exit`
  - `GREEN`, 6/6 reachable, height `1129`, root
    `2dd55fa0da79cf11a04ecc55c15bd9dbf054fedfbfd0a98d20f030c99556b44b2beef3cec2962ed05df19221f8328b21`,
    mempool 0.

Next improvement item:

- Attack the remaining submit/finality tail. The current evidence points at
  deterministic proposer parent catch-up and certified-round transport rather
  than quote reads. The next optimization pass should reduce
  `submit_finality_ms` and finality `readiness_wait_ms` without returning to
  unsafe optimistic routing.

### 2026-06-28T18:00Z - 2026-06-28T18:12Z Persistent Vote Streams And RPC Upstream Keep-Alive

Status: incremental Stage 8 improvement deployed to the WAN devnet. This pass
removed the per-vote TCP handshake from warmed certified rounds and promoted
wallet-proxy upstream RPC keep-alive after live A/B showed it now improves the
account-lane wallet path.

Evidence:

- Persistent vote-stream binary rollout:
  `reports/transaction-improvement/20260628T180026Z-persistent-vote-streams-deploy/`
- Post-deploy strict preflight:
  `reports/transaction-improvement/20260628T180026Z-persistent-vote-streams-deploy/post-deploy-preflight.json`
- Persistent vote-stream smoke:
  `reports/transaction-improvement/20260628T180217Z-persistent-vote-streams-account-smoke/latency-summary.md`
- Persistent vote-stream warm smoke:
  `reports/transaction-improvement/20260628T180257Z-persistent-vote-streams-warm-account-smoke/latency-summary.md`
- Wallet-proxy upstream keep-alive smoke:
  `reports/transaction-improvement/20260628T180637Z-persistent-vote-and-rpc-keepalive-account-smoke/latency-summary.md`
- Wallet-proxy upstream keep-alive warm smoke:
  `reports/transaction-improvement/20260628T180711Z-persistent-vote-and-rpc-keepalive-warm-account-smoke/latency-summary.md`
- Post-smoke strict preflight:
  `reports/transaction-improvement/20260628T180026Z-persistent-vote-streams-deploy/post-keepalive-smoke-preflight.json`

Actions completed:

- Changed `transport-validator-serve` from a single-threaded one-request
  accept loop to a worker-per-connection service that can process multiple
  newline-delimited requests on one TCP stream.
- Added a process-local validator vote stream pool for
  `transport_block_vote_request`. Pooled vote streams are enabled by default
  and can be disabled with `POSTFIAT_TRANSPORT_PERSISTENT_VOTE_STREAMS=0`.
  Stale pooled streams are dropped and retried through the existing fresh TCP
  path.
- Preserved the existing transport request, vote, certificate, and receipt
  formats. The change is transport reuse only; it does not alter block hashes,
  proposal hashes, state roots, receipts, or certificates.
- Built and deployed `target/release/postfiat-node` hash
  `6f4df850a5c2497b6e15d582db35970ed15b9f0ecfab1eeca8f1e8790a8cd8d5` to all
  six validators. Each validator backed up prior hash
  `07c17813fcc3dc1f31291b56823a3566aab0500685b5fd163ce110205c05e84d`.
- Restarted the wallet-facing proxy on port `8080` with upstream RPC
  keep-alive enabled and changed `wallet-proxy/server.js` so
  `ENABLE_UPSTREAM_KEEPALIVE` defaults to enabled. Rollback is
  `ENABLE_UPSTREAM_KEEPALIVE=false`.

Measured result:

- Before this pass, the TCP_NODELAY smoke measured native PFT p50
  `2008.132 ms` and memo `payment_v2` p50 `2001.766 ms`.
- With warmed persistent validator vote streams and one-shot proxy upstreams,
  native PFT p50 improved to `1646.683 ms` and memo `payment_v2` p50 improved
  to `1717.362 ms`.
- Round-report inspection confirmed warmed vote requests reused connections:
  `48/48` vote-request `transport_connect_ms` values were `0.0 ms` in the
  warm smoke. Certified-round p50 was about `614.650 ms`, vote-request p50 was
  about `444.393 ms`, and submit/finality p50 was about `1201.349 ms`.
- With wallet-proxy upstream RPC keep-alive enabled, the first keep-alive smoke
  measured native PFT p50 `1271.898 ms` and memo `payment_v2` p50
  `1556.630 ms`. The second warm keep-alive smoke measured native PFT p50
  `1595.102 ms`, memo `payment_v2` p50 `1565.380 ms`, and max under
  `1858.450 ms` / `1853.849 ms` for the two categories.
- Post-smoke strict preflight remained green: all six validators at height
  `1189`, identical state root
  `1ad0dc81b96f99a54460db109fcd3695c7d4151e227c92b12b6777b93bf8abf8f751943b0eb66994a4112759436374cb`,
  and mempool `0`.

Checks run:

- `cargo fmt --check`
- `cargo check -p postfiat-node`
- `cargo test -p postfiat-node --all-targets transport_batch_payload_tests`
- `cargo test -p postfiat-node --all-targets rpc_proxy_parent_wait_is_limited_to_quote_methods`
- `cargo test -p postfiat-node --all-targets mempool_submit_signed_payment_v2_finality_allowed_under_finality_flag`
- `node --check wallet-proxy/server.js`
- `node wallet-proxy/test_proposer_routing.js`
- `PYTHONPATH=python python3 -m pytest python/tests/test_latency.py python/tests/test_wallet.py python/tests/test_wan_preflight.py -q`
  - `65 passed`

Next improvement item:

- Continue reducing the remaining account-lane tail. The vote-stream
  handshake is removed; the next largest visible costs are cross-region
  proxy-to-proposer RPC RTT for slow proposers and the still-required
  cross-region quorum vote. Candidate next work: run the wallet proxy close to
  the active proposer region or reset/redeploy the controlled devnet to a
  low-latency validator topology, then rerun the same Stage 8 smoke and the
  larger 50/50/20 gate.

### 2026-06-28T18:51Z - 2026-06-28T18:58Z FastPay Hot-Send Measurement And Browser Vote Parallelism

Status: incremental Stage 6 and Stage 8 improvement. This pass separates the
FastPay hot send from account-to-owned wrapping and owned-to-account
unwrapping, and aligns the browser wallet with the Python helper by collecting
FastPay validator votes in parallel.

Evidence:

- FastPay send-only smoke:
  `reports/transaction-improvement/20260628T185143Z-fastpay-send-only-hot-smoke/latency-summary.md`
- Raw per-step send-only timings:
  `reports/transaction-improvement/20260628T185143Z-fastpay-send-only-hot-smoke/latency-raw.jsonl`

Actions completed:

- Added `--fastpay-send-only-count` to `scripts/wan-devnet-latency-run`.
  The runner now pre-wraps a fresh owned object as setup, records that setup
  under the event's `setup` block, and times only the owned-object
  send/apply path in the `fastpay_send_only` category.
- Changed `wallet-web/src/lib/tx-builder.js` so browser FastPay sends request
  validator votes concurrently and return once the Byzantine quorum is
  collected. The browser path no longer waits serially through validators or
  stalls on a slow sixth validator after quorum is available.
- Added regression coverage for the send-only latency event and the browser
  quorum path with a non-returning sixth validator.

Measured result:

- Live `fastpay_send_only` smoke completed `5/5` with zero failures.
- Hot send p50 was `850.118 ms`; max was `1476.720 ms`.
- The raw timing split shows setup wrap time is excluded from `duration_ms`.
  In the warm samples, owner signing was about `72-93 ms`, quorum vote
  collection was about `276-294 ms`, and `owned_apply` broadcast was about
  `440-685 ms` after the first sample.

Next improvement item:

- Reduce the remaining `owned_apply` broadcast cost without reintroducing
  split owned-object state. Candidate paths are a certified owned-object batch
  type, a background reconciliation/catch-up path after quorum-fast apply, or a
  low-latency controlled validator topology for the wallet-facing FastPay
  fleet.

### 2026-06-28T19:02Z - 2026-06-28T19:08Z Server-Info Resilience And Full Stage 8 Sample

Status: Stage 8 account-lane and FastPay hot-send sample completed on the live
WAN devnet after fixing an RPC health/capability defect. The remaining Stage 8
work is broader permutation coverage for the Stage 7 write paths that are not
yet enabled on the wallet-facing endpoint.

Evidence:

- Server-info fallback preflight:
  `reports/transaction-improvement/20260628T190259Z-server-info-fallback-preflight/preflight.json`
- Full Stage 8 latency sample:
  `reports/transaction-improvement/20260628T190327Z-stage8-full-after-server-info-fallback/latency-summary.md`
- Raw full-sample timing events:
  `reports/transaction-improvement/20260628T190327Z-stage8-full-after-server-info-fallback/latency-raw.jsonl`
- Post-run strict preflight:
  `reports/transaction-improvement/20260628T190327Z-stage8-full-after-server-info-fallback/post-run-preflight.json`

Actions completed:

- Fixed `server_info` so wallet-critical RPC capability discovery survives a
  metrics/archive parse failure. `status` remains required; metrics are now
  reported as `{ ok: false, error: ... }` with a
  `server_info_metrics_unavailable` warning instead of taking the entire RPC
  endpoint red.
- Added a Rust regression proving the fallback response remains valid under
  the RPC SDK and still returns active validator count plus nonzero fee
  constants.
- Built and deployed release binary
  `c42bc94b98ead3ca5c81e7cbf0cdd1b0f8ef8ed1f46c1265a06072b8d130b3d4`
  to validators 0 through 5, then restarted each validator and RPC unit.
- Verified validator-0 `server_info` now returns successfully even though
  metrics still reports the malformed archive line as a warning.
- Verified strict preflight green before the run at height `1221`, root
  `9e234d3793b8c5a1354c62cc7341184209b68f2820a53c9f8e68a1154b7557b2c4ae2aa9ffcd6bd1cdcf5e4fbd198f43`,
  with 6/6 validators reachable and mempool `0`.
- Ran the full account-lane/FastPay-hot Stage 8 sample: `50` native PFT
  sends, `50` memo `payment_v2` sends, and `20` FastPay send-only transfers.
- Verified strict preflight green after the run at height `1321`, root
  `961f651c6e453d601548f62266b5411c71cc47ff8c85e5402244f5fa1f790d8f9c2af1cc7d39244f5b08aa8a4a3a3227`,
  with 6/6 validators reachable and mempool `0`.

Measured result:

- Native PFT: `50/50` succeeded, p50 `1634.384 ms`, p90 `1931.202 ms`,
  p95 `2238.193 ms`, p99/max `5006.811 ms`.
- Memo `payment_v2`: `50/50` succeeded, p50 `1592.785 ms`, p90
  `1887.342 ms`, p95 `1918.793 ms`, p99/max `2046.557 ms`.
- FastPay send-only: `20/20` succeeded, p50 `711.129 ms`, p90
  `1121.317 ms`, p95 `1182.532 ms`, p99/max `1593.690 ms`.

Next improvement item:

- Keep reducing p95/p99 tails on the account lane and FastPay `owned_apply`.
  The next concrete options are low-latency validator placement, quorum-fast
  owned-object reconciliation, and enabling the currently disabled Stage 7
  trustline/asset/offer/Orchard/bridge write paths for measured coverage.

### 2026-06-28T19:15Z - 2026-06-28T19:23Z Browser Wallet Routing And Quote Timeout Fix

Status: browser wallet native-send smoke now completes through the live WAN
wallet proxy with a final receipt and without the 10-second quote retry. This
advances Stage 9 for native PFT browser sends; memo-send, FastPay browser
click-path, receipt-detail lookup, and the broader Stage 7 transaction matrix
remain active work.

Evidence:

- Slow browser smoke before the proxy cap/id fix:
  `reports/transaction-improvement/20260628T191857Z-wallet-browser-live-smoke/wallet-browser-live-smoke.json`
- WebSocket probe showing the first `transfer_fee_quote` request waited for the
  browser RPC timeout before retrying:
  `reports/transaction-improvement/20260628T192033Z-wallet-browser-live-smoke/wallet-browser-live-smoke.json`
- Improved browser smoke after the fix:
  `reports/transaction-improvement/20260628T192233Z-wallet-browser-live-smoke/wallet-browser-live-smoke.json`
- Final screenshot:
  `reports/transaction-improvement/20260628T192233Z-wallet-browser-live-smoke/wallet-send-accepted.png`

Actions completed:

- Fixed the Vite wallet default RPC endpoint so local dev pages on `5173`
  connect directly to the wallet proxy on `8080` instead of relying on a Vite
  WebSocket proxy path.
- Made wallet dev HTTPS explicit opt-in. Existing `/tmp/vite-key.pem` and
  `/tmp/vite-cert.pem` files no longer silently turn the local wallet into an
  HTTPS page that tries to talk `wss://` to a plain local proxy.
- Changed sequenced account reads in `wallet-proxy/server.js` so cached
  post-finality read routes accept any validator at or beyond the required
  parent height, while exact height/root matching remains reserved for finality
  proposal routes.
- Raised the per-wallet proxy TCP concurrency bound from `10` to `32` so React
  dev hydration reads do not starve the first user action.
- Moved the proxy rate-limit check after JSON parsing and request validation so
  a capped request receives a response with its original RPC id. The browser no
  longer leaves the pending call unresolved until the client-side timeout.

Measured result:

- Before: browser native send succeeded, but `Review send` took
  `10481.917 ms`; the first quote request was capped without a matching id and
  the browser retried after its 10-second timeout.
- After: browser native send succeeded with `Review send` in `78.615 ms` and
  `Confirm and Sign` to final receipt in `1873.868 ms`.
- The improved smoke had zero page errors, zero console errors, no raw
  `RPC send failed` text, and no pending-finality success placeholder.

Next improvement item:

- Add the remaining browser Stage 9 coverage: memo `payment_v2` click-path,
  FastPay wrap/send/unwrap click-path, and receipt-detail lookup. Keep the same
  hard gate: browser evidence must show a final receipt, no raw RPC failure
  text, and measured timings for quote/review and confirm/finality.

## Stage Gates

Each stage has a completion-note slot. The overnight agent must fill the slot
with the evidence path, commands run, final status, and remaining risk before
moving on.

### Stage 0: Freeze And Snapshot

Goal: stop uncontrolled transaction attempts and preserve current evidence.

Tasks:

- Stop any running ad-hoc catch-up or transaction repair process.
- Record active validator/RPC processes on all six WAN validators.
- Record current block height, state root, mempool count, binary hash, service
  unit, and RPC capabilities for each validator.
- Snapshot each validator data dir before reset or repair.
- Save the failed browser transaction, finality report, validator-1 rejection
  logs, and any catch-up logs into a dated report directory.

Gate:

- `reports/transaction-improvement/<timestamp>/fleet-baseline.json` exists.
- Six validator process inventories exist.
- Current inconsistent state is documented, not inferred.

Completion note:

```text
Status:
Evidence:
Commands:
Residual risk:
```

### Stage 1: Fleet Binary And Service Convergence

Goal: all validators and RPC services run the same intended binary and service
configuration.

Tasks:

- Build a release binary from the intended commit.
- Record local binary hash and git commit.
- Install the same binary on all six validators.
- Restart validator and RPC services in a controlled order.
- Verify `server_info.rpc` is produced by the node itself and matches the
  active flags.
- Remove or disable proxy-side capability injection unless it is derived from
  live upstream capability checks.
- Verify all six validators report the same binary hash.

Gate:

- All six validators report the same `/usr/local/bin/postfiat-node` hash.
- All six service command lines match the intended finality configuration.
- `server_info.rpc.mempool_submit_finality_enabled` is true only when the
  endpoint actually accepts the finality method that the wallet will call.

Completion note:

```text
Status:
Binary hash:
Git commit:
Validators updated:
Evidence:
Residual risk:
```

### Stage 2: Network Reset Or Repair Decision

Goal: choose a clean state strategy instead of force-syncing blindly.

Decision rule:

- Repair in place only if all validators have the same height/root or are
  exactly one verified block behind and can catch up through the supported
  catch-up command in under 60 seconds.
- Reset the controlled devnet if validators have divergent mempools, repeated
  proposal-hash mismatches, catch-up stalls, inconsistent binaries, or missing
  proposer/finality keys.

Reset requirements if selected:

- Preserve snapshots.
- Generate a fresh topology and registry.
- Ensure every validator owns its own validator key and the finality service
  can either route to the current proposer or submit through a non-proposer
  path.
- Start with empty mempools.
- Produce a genesis/fleet report.

Gate:

- A written decision exists: `repair` or `reset`.
- If reset: new chain id/genesis hash/topology are recorded.
- If repair: every validator is converged at the same height/root and can
  accept the next certified block.

Completion note:

```text
Status:
Decision:
Rationale:
Evidence:
Residual risk:
```

### Stage 3: Finality API Contract

Goal: define and enforce one canonical wallet transaction finality contract.

Tasks:

- Decide the canonical browser send method for native PFT without memo.
- Decide the canonical browser send method for memo-bearing `payment_v2`.
- Ensure both methods either use finality RPC or are explicitly rejected until
  finality is available.
- Remove silent fallbacks from finality submit to mempool-only submit.
- Make finality errors distinguish:
  - method disabled;
  - wrong proposer;
  - quorum failure;
  - proposal hash mismatch;
  - stale local state;
  - mempool duplicate;
  - transaction rejected by state transition.
- Return a final receipt or a typed failure. Do not return "pending finality"
  unless a background finality process is actually running and observable.

Gate:

- Unit tests prove wallet/native and wallet/payment-v2 do not silently fall
  back to mempool-only submission.
- RPC tests prove disabled finality is visible as a typed capability failure.
- A live WAN test finalizes one native PFT transfer and one memo
  `payment_v2` transfer through the wallet-facing endpoint.

Completion note:

```text
Status:
Methods:
Evidence:
Residual risk:
```

### Stage 4: Proposer-Aware Routing Or Submit-Forward

Goal: any wallet-facing send must work without the user knowing which
validator is the next proposer.

Acceptable implementations:

- Wallet proxy routes finality requests to the current proposer.
- RPC server forwards signed transactions to the current proposer.
- Validators gossip mempool entries and the proposer seals them.
- Consensus loop continuously seals mempool transactions without a special
  finality RPC per send.

Required behavior:

- If height is `H`, the router computes the proposer for `H+1`.
- It verifies that proposer endpoint is healthy before sending.
- It retries on view/proposer changes without double-submitting.
- It returns the final receipt from the committed block.
- It records route, proposer, height, tx id, and timings.

Gate:

- Send from a wallet-facing endpoint succeeds for at least six consecutive
  blocks, including proposer changes.
- The client never needs to be reconfigured manually from validator-1 to
  validator-4 or similar.
- Route evidence proves which proposer accepted each transaction.

Completion note:

```text
Status:
Routing mode:
Evidence:
Residual risk:
```

### Stage 5: Python Tooling Parity

Goal: Python helpers match the real wallet/WAN transaction path.

Tasks:

- Audit all Python helpers that can send or finalize transactions.
- Split helper modes explicitly:
  - `local_harness_apply_batch`;
  - `wan_submit_finality`;
  - `wan_submit_and_poll`;
  - `fastpay_object_flow`.
- Rename or reject any helper that defaults to local harness behavior while
  bypassing WAN finality.
- Add `request_faucet_pft`, `send_pft`, `send_pft_and_poll_finality`,
  `wrap_fastpay`, `unwrap_fastpay`, and `send_fastpay` helpers with sane
  defaults for the controlled WAN devnet.
- Ensure helpers discover chain id, genesis hash, protocol version, sequence,
  fees, and RPC capabilities live from RPC.
- Ensure helpers record receipt, block height, state root, and endpoint.

Gate:

- Python can perform the same native PFT wallet send path as the browser.
- Python can perform the same memo `payment_v2` path as the browser.
- Python can perform FastPay wrap/send/unwrap.
- Tests fail if a WAN helper uses local `apply-batch` without an explicit
  `local_harness_apply_batch` mode.

Completion note:

```text
Status: partially complete for native PFT, memo payment_v2, and FastPay
through the wallet-facing WAN proxy. Broader asset/offer/Orchard helper
parity is still covered by Stage 7.
Helpers fixed: Python helpers now separate WAN finality from local harness
apply-batch; FastPay helpers require a broadcast-capable wallet endpoint and
reject raw single-validator RPC for wrap/apply/unwrap.
Tests: python/tests/test_wallet.py and python/tests/test_wan_preflight.py
passed with 54 tests.
Live evidence: native and payment_v2 evidence is in the 2026-06-28T02:21Z
execution log. FastPay helper evidence is
reports/transaction-improvement/20260628T022227Z-fleet-repair/fastpay-live/python-websocket-fastpay-cycle-20260628T0330Z.json.
Residual risk: FastPay Python parity currently means WebSocket proxy parity,
not consensus-native FastPay block finality.
```

### Stage 6: FastPay End-To-End

Goal: FastPay is a working lane, not a broken UI tab.

Tasks:

- Define the FastPay object model in wallet docs and RPC docs.
- Make account balance and FastPay balance separate and visible.
- Ensure `owned_objects` works against the wallet-facing endpoint.
- Resolve recipient public keys safely:
  - raw public key hex accepted;
  - own account address maps to own public key;
  - account address with published public key accepted;
  - account address without public key rejected with clear copy.
- Test wrap account balance to FastPay object.
- Test send FastPay object to another wallet.
- Test unwrap FastPay object back to account balance.
- Confirm final receipts and balance deltas.
- Make the UI show actionable errors instead of `RPC send failed:
  owned_objects`.

Gate:

- Fresh wallet A wraps `10 PFT` into FastPay.
- Wallet A sends a smaller test amount to wallet B.
- Wallet B sees FastPay balance.
- Wallet B unwraps to account balance.
- All steps have receipts and no raw RPC error text in the UI.

Completion note:

```text
Status: controlled-devnet wallet-facing FastPay path works end to end through
the proxy-broadcast mode. Not yet consensus-native finality.
Wallet A: pfa95c2c765a41b24867b23703ac688d9eaa8a9264 wrapped fresh 2 PFT
objects and sent 1 PFT FastPay objects in two fresh cycles.
Wallet B: pf65c9783ceafc0f519a74195e78cc7909f92429c3 received and unwrapped
FastPay objects; balance moved from 2.000740 PFT to 4.000740 PFT across the
fresh proxy and Python-helper cycles.
Receipts: proxy broadcast reports show 6/6 application for wrap/apply/unwrap
and owned-transfer apply reports show node-side quorum 5 of 6.
Latency: fresh proxy cycle showed wrap 1.590s, vote collection 5.689s, apply
2.100s, unwrap 1.574s. Python helper cycle completed in about 22.6s end to
end including capability checks and state reads.
Residual risk: FastPay bridge mutations still bypass canonical block
certification and leave the block height/state root unchanged. Move
wrap/unwrap/apply into a certified transaction or batch before treating FastPay
as a finalized low-latency lane.
```

### Stage 7: Transaction Permutation Matrix

Goal: prove the L1 transaction layer works across real transaction classes.

Required matrix:

| Category | Required operations |
| --- | --- |
| Native account | create account, fund, PFT transfer, memo payment_v2, sequence conflict, insufficient funds |
| Trustlines/assets | trustline set, issued asset transfer, asset fee quote, account lines, account assets |
| Offers/atomic | offer create, offer fill, atomic settlement template, cancel/expiry path |
| FastPay | wrap, owned object lookup, object send, unwrap, duplicate/replay rejection |
| Orchard | keygen, deposit/ingress, spend, withdraw, nullifier replay rejection |
| Asset-Orchard | ingress, private swap, egress/private egress where enabled, invalid proof rejection |
| Bridge batches | bridge domain status, bridge transfer batch, vault bridge certified ops where enabled |
| Governance-safe no-op | governance batch rejected/accepted as appropriate under test fixture |

For each operation, record:

- command or API path;
- RPC endpoint;
- signed payload hash;
- tx id or batch id;
- block height;
- state root before/after;
- receipt;
- latency breakdown;
- whether the operation is wallet-supported today.

Gate:

- The matrix is checked into `reports/transaction-improvement/<timestamp>/`.
- At least one live accepted receipt exists for every enabled category.
- Disabled categories fail with explicit, documented errors.

Completion note:

```text
Status: partial. A repeatable Stage 7 matrix command now exists and was run
against the wallet-facing WAN proxy, but the full matrix is not complete.
Matrix path: reports/transaction-improvement/20260628T022227Z-fleet-repair/stage7-transaction-matrix.json
and reports/transaction-improvement/20260628T022227Z-fleet-repair/stage7-transaction-matrix.md.
Accepted categories: Native account is accepted_partial from existing
wallet-proxy native and payment_v2 finality evidence. FastPay is
accepted_controlled_devnet_proxy from proxy-broadcast evidence.
Disabled categories: Trustlines/assets and Offers/atomic write methods return
rpc_method_not_allowed on the wallet-facing endpoint. Orchard and Bridge
batches expose read-only status but wallet-facing batch write methods are
disabled.
Residual risk: Native conflict/failure permutations, trustline/asset writes,
offer/atomic writes, Orchard spend/withdraw/replay, Asset-Orchard actions,
bridge batches, and governance-safe no-op remain unproven live.
```

### Stage 8: Latency Measurement And Optimization Evidence

Goal: improve slow latency paths until measured evidence reaches the target.

Tasks:

- Collect local devnet latency separately from WAN devnet latency.
- Measure:
  - quote time;
  - signing time;
  - RPC submit time;
  - proposer routing time;
  - block proposal time;
  - vote collection time;
  - certificate build time;
  - local apply time;
  - certified send/propagation time;
  - receipt availability time;
  - UI click-to-receipt time.
- Run at least:
  - 50 native PFT sends;
  - 50 memo `payment_v2` sends;
  - 20 FastPay sends;
  - representative trustline/asset/offer transactions;
  - representative Orchard/Asset-Orchard operations where proof time is
    expected to dominate.
- Report p50, p90, p95, p99, max, and failure rate.
- Compare against current website/blog latency targets.
- Produce an improvement list for targets that are not met.

Gate:

- A latency report exists with raw data and summary.
- Every latency target is mapped to current evidence or an active improvement
  task.
- No latency measurement combines manual recovery time with hot-path finality
  time.

Completion note:

```text
Status: account-lane and FastPay hot-send full sample complete after fixing,
deploying, and measuring a controlled RPC server-info defect. The repeatable command is
scripts/wan-devnet-latency-run. It gates on fleet preflight and RPC submit
capability before mutable probes and writes latency-raw.jsonl,
latency-summary.json, and latency-summary.md. A red RPC capability gate is not
a valid terminal outcome for this sprint: the WAN devnet RPC services are
project-controlled infrastructure, so the agent must repair the node/proxy/RPC
configuration, restart or roll the fleet as needed, and rerun the latency gate.
Latency reports:
reports/transaction-improvement/20260628T022227Z-fleet-repair/stage8-account-latency-breakdown-20260628T121259Z/latency-summary.md
and
reports/transaction-improvement/20260628T022227Z-fleet-repair/stage8-latency-smoke-20260628T120833Z/latency-summary.md.
The attempted full 50 native / 50 memo payment_v2 / 20 FastPay sample at
reports/transaction-improvement/20260628T022227Z-fleet-repair/stage8-latency-full-20260628T121430Z/latency-summary.md
did not pass: native reached 43/50, memo payment_v2 reached 0/50, and FastPay
reached 20/20. The account-lane failures were process-lifetime RPC submit caps
and temporary proposer-routing convergence failures, not wallet signing errors.
The runner now fails fast to avoid burning more transactions when the live
proxy advertises only max_mempool_submit_per_peer=16 and
max_mempool_submit_total=64:
reports/transaction-improvement/20260628T022227Z-fleet-repair/stage8-latency-full-blocked-rate-limit-20260628T123946Z/latency-summary.md.
That fail-fast report is only evidence for the next fix. It is not completion.
Because we control the RPC services, the required next action is to deploy a
bounded rolling-window RPC limiter or benchmark-safe RPC submit profile, restart
the RPC fleet, verify server_info exposes the live window/caps, and rerun the
50/50/20 sample.
The account-lane smoke measured one wallet-facing native PFT send and one
memo payment_v2 send with quote/sign/finality-submit split: native client wall
3539.315 ms (quote 840.520 ms, sign 52.016 ms, submit/finality 2645.576 ms,
node total 958.221137 ms, certified round 931.445522 ms), payment_v2 client
wall 3097.793 ms (quote 789.518 ms, sign 70.063 ms, submit/finality
2237.260 ms, node total 803.594933 ms, certified round 781.438007 ms). The
combined smoke also measured one FastPay proxy-broadcast cycle at 18954.546 ms
(wrap 4029.958 ms, send/apply 11154.043 ms, unwrap 3770.426 ms). Post-run
preflight is
reports/transaction-improvement/20260628T022227Z-fleet-repair/post-stage8-account-latency-preflight.json,
GREEN at height 493 with 6/6 validators converged and empty mempools.
Optimization validated so far: FastPay hot-path work cut the controlled-devnet
proxy-broadcast wrap/send/unwrap p50 from about `20.0s` to about `3.297s`.
The browser wallet confirm path was also fixed so `Send Now` reuses the
reviewed quote instead of issuing a second `transfer_fee_quote` RPC before
signing and submitting. Current wallet-confirm smoke evidence:
reports/transaction-improvement/20260628T022227Z-fleet-repair/stage8-wallet-confirm-current-smoke-20260628T152456Z/latency-summary.json.
That sample measured native PFT Review quote p50 `961.699 ms`, native
Confirm/Send Now sign+submit p50 `1365.849 ms`, memo `payment_v2` Review quote
p50 `1757.185 ms`, and memo `payment_v2` Confirm/Send Now sign+submit p50
`1758.414 ms`. This is progress, not a completed latency gate.
First-ready sequenced-read routing was measured as an experiment, not promoted
to the default proxy mode. Immediate-confirm evidence:
reports/transaction-improvement/20260628T022227Z-fleet-repair/stage8-first-ready-quote-smoke-20260628T153024Z/latency-summary.json.
Human-review evidence with `--review-delay-ms 2000`:
reports/transaction-improvement/20260628T022227Z-fleet-repair/stage8-first-ready-review-delay-smoke-20260628T153220Z/latency-summary.json.
It reduced some quote tails, but still produced submit-route waits up to about
`1.6s` on the immediate path and did not fully remove them with a `2s` review
pause. The live wallet proxy therefore remains on proposer-routed sequenced
reads by default while quote/state-read latency remains an active optimization
target.
Optimization evidence:
reports/transaction-improvement/20260628T022227Z-fleet-repair/latency-optimization-evidence.md.
Performance gaps to improve: the current browser wallet path is still not
subsecond; quote/state-read tails are still too high on some proposers; the
current wallet-facing FastPay wrap/send/unwrap UX does not meet the historical
`183 ms` FastPay target; old local benchmark paths must be rerun against the
current code and fleet before they count as achieved targets.
Residual risk: FastPay measurements are proxy-broadcast controlled-devnet
timings, not consensus-native block finality; trustline/asset/offer/Orchard/
bridge latency remains blocked by the disabled Stage 7 write paths.
Current completed full sample:
reports/transaction-improvement/20260628T190327Z-stage8-full-after-server-info-fallback/latency-summary.md.
That sample completed 50/50 native PFT, 50/50 memo payment_v2, and 20/20
FastPay send-only transfers with zero failures. Post-run preflight:
reports/transaction-improvement/20260628T190327Z-stage8-full-after-server-info-fallback/post-run-preflight.json.
```

Mandatory next step:

- Fix RPC, do not merely report it. Since PFTL controls the validator RPC
  services, `rpc_mempool_submit_rate_limited`, stale process-lifetime counters,
  misleading `server_info`, and proposer-routing misses are owned defects.
  Acceptable work includes code changes, config changes, fleet restart, rolling
  deploy, state repair, or devnet reset if needed by the gate. Unacceptable work
  is leaving controlled RPC/network defects unimproved while non-reserved
  remediation options remain.

### Stage 9: Wallet UI Status And Recovery Optimization

Goal: the wallet shows actionable transaction state and recovers quickly.

Tasks:

- Replace generic `offline`, `pending finality`, and raw RPC messages with
  typed states:
  - offline;
  - read-only;
  - writable but no finality;
  - finality route unavailable;
  - proposer unhealthy;
  - quorum unavailable;
  - pending with observable job id;
  - finalized;
  - rejected.
- Show account balance and FastPay balance separately.
- Show the current RPC endpoint and network height.
- Add a "resync status" action.
- Add a transaction detail panel that can query `tx` and `receipts` by id.
- Ensure the wallet never reports success unless a final receipt is present.

Gate:

- Browser smoke test covers create wallet, unlock, receive/fund, native send,
  memo send, FastPay send, and receipt lookup.
- Zero page errors.
- No raw `RPC send failed` strings are visible to the user without a human
  translation.

Completion note:

```text
Status:
Browser evidence:
Known UX gaps:
Residual risk:
```

### Stage 10: Validator Liveness And Release Gate

Goal: prevent another binary/config/state split from reaching the wallet.

Tasks:

- Add a fleet preflight command that checks all validators:
  - service active;
  - binary hash;
  - git/build id if available;
  - height/root;
  - mempool count;
  - RPC methods enabled;
  - current and next proposer health;
  - vote request health;
  - certified batch send health.
- Make transaction tests refuse to run if fleet health is red.
- Add post-deploy health verification after each validator update.
- Add rollback command and documented rollback threshold.
- Add a "no mixed binary fleet" guard.

Gate:

- A single command produces a green/red fleet report.
- The report blocks transaction acceptance tests when fewer than five
  validators are healthy and converged.
- Rollout procedure verifies quorum after each validator update.

Completion note:

```text
Status:
Preflight command:
Rollback command:
Evidence:
Residual risk:
```

### Stage 11: Documentation And Operator Runbook

Goal: operators can run, diagnose, and repair the transaction layer without
guesswork.

Tasks:

- Update RPC docs with the canonical transaction finality contract.
- Update Python docs with helper modes and warnings.
- Update wallet docs with account/FastPay balance model.
- Update validator runbooks with fleet reset, binary rollout, catch-up, and
  rollback procedures.
- Add a postmortem for this incident.
- Add a short "what not to trust" section:
  - local `apply-batch` is not WAN proof;
  - mempool admission is not finality;
  - RPC capability metadata must be live;
  - one validator receipt is not fleet convergence.

Gate:

- MkDocs builds.
- Docs point to current commands and not stale helper paths.
- Every operational requirement has an evidence pointer.

Completion note:

```text
Status:
Docs changed:
MkDocs:
Residual risk:
```

## Overnight Execution Order

1. Freeze, snapshot, and write the fleet baseline.
2. Converge binaries and service units.
3. Decide repair versus reset. Reset is authorized if repair is not clean.
4. Establish the finality API contract.
5. Implement proposer-aware routing or submit-forward.
6. Bring Python helpers into parity with wallet WAN behavior.
7. Fix and prove FastPay.
8. Run the transaction permutation matrix.
9. Run latency measurements and optimize missed targets.
10. Fix wallet status and receipt UX.
11. Add fleet liveness gates and operator docs.

The job should not stop after a single successful transaction. A single green
send only proves one route at one height. The exit condition is a matrix of
transaction types, multiple proposer rotations, measured latency, and
consistent validator state.

## Acceptance Definition

The transaction layer is acceptable only when all of the following are true:

- Six validators are on the intended binary and service configuration.
- The wallet-facing endpoint can finalize native PFT and memo `payment_v2`
  transactions without manual endpoint switching.
- Browser wallet, Python helper, and direct RPC runs produce equivalent
  receipts for the same transaction categories.
- FastPay works end-to-end with visible account and object balances.
- Trustline, asset, offer/atomic, Orchard, Asset-Orchard, and bridge-related
  permutations have current evidence or explicit disabled-status docs.
- Latency reports show current performance and active improvements for missed
  targets.
- Fleet health gates prevent transaction tests from running on a degraded
  network.
- No manual validator force-sync is part of the normal transaction path.
- The docs explain exactly what broke and exactly how to avoid repeating it.

## Required Evidence Package

The overnight job must produce a final packet under:

```text
reports/transaction-improvement/<timestamp>/
```

Minimum contents:

- `fleet-baseline.json`
- `binary-rollout.json`
- `reset-or-repair-decision.md`
- `fleet-health-after.json`
- `wallet-native-send.json`
- `wallet-payment-v2-send.json`
- `python-native-send.json`
- `python-payment-v2-send.json`
- `fastpay-wrap-send-unwrap.json`
- `transaction-permutation-matrix.json`
- `latency-raw.jsonl`
- `latency-summary.md`
- `latency-optimization-evidence.md`
- `wallet-playwright-report.md`
- `operator-runbook-diff.md`
- `completion.md`

The `completion.md` file must state what was proven, what remains unproven,
and whether the network was reset.

## Agent Completion Template

Every agent handoff must append:

```text
Timestamp:
Commit:
Current stage:
Completed stages:
Blocked stages:
Validator fleet state:
Wallet endpoint:
Python helper state:
FastPay state:
Latency evidence:
Next command:
Do not do:
```

The `Do not do` line must call out any action that would waste time, such as
re-running wallet sends against a known red fleet or using local `apply-batch`
as proof of WAN finality.

## 2026-06-28 RPC Parent-Wait Cleanup

The synchronous next-proposer certified-send experiment reduced one visible
proxy wait but added network send time to the certified round itself. It was
removed from the source tree instead of carried forward as a dormant flag.

The replacement path is:

1. After a finalized block, the wallet proxy caches the next deterministic
   proposer and the exact parent height/state root it must build on.
2. A subsequent wallet finality request is routed to that proposer without a
   foreground proxy status loop.
3. The proxy annotates the request with `proxy_required_current_height`,
   `proxy_required_state_root`, and `proxy_readiness_timeout_ms`.
4. The node RPC checks its own local state and waits for that exact parent
   before proposing. If it cannot reach the required parent in time, it returns
   `rpc_finality_parent_not_ready` instead of proposing on the wrong root.
5. The RPC response includes `readiness_wait_ms` so latency reports separate
   parent catch-up from mempool batching and certified finality.

This keeps the already-working deferred certified-send behavior and removes the
unsafe/slow proxy polling tail from the normal wallet path. The next deployment
must replace the fleet binary that still contains the synchronous send
experiment, then rerun the latency gate and compare `proxy_route.route_wait_ms`
against RPC `readiness_wait_ms`.

Evidence:

- Pre-deploy gate:
  `reports/transaction-improvement/20260628T165420Z-pre-rpc-parent-wait-deploy.json`
- Deployment packet:
  `reports/transaction-improvement/20260628T165456Z-rpc-parent-wait-deploy/`
- Post-deploy gate:
  `reports/transaction-improvement/20260628T165600Z-post-rpc-parent-wait-deploy.json`
- 12-send account smoke:
  `reports/transaction-improvement/20260628T165638Z-rpc-parent-wait-account-smoke/`
- Readiness-metric smoke:
  `reports/transaction-improvement/20260628T165842Z-rpc-parent-wait-readiness-smoke/`
- Post-smoke gate:
  `reports/transaction-improvement/20260628T165913Z-post-rpc-parent-wait-smoke-preflight.json`

Measured result: wallet finality routes use
`readiness_check=rpc_parent_wait_finality_route`, `proxy_route.route_wait_ms`
is `0` on the smoke samples, and node-side `readiness_wait_ms` is now recorded
in latency raw evidence. Remaining latency work moves to quote/read latency,
certified-round time, FastPay, and the full transaction permutation matrix.

## 2026-06-29 FastPay Standard Unwrap, 2048 Input Cap, And Redeploy

Status: wallet-facing FastPay standard unwrap is implemented in wallet and
Python tooling and the node-side protocol cap has been redeployed across the
six-validator WAN devnet.

What changed:

- `unwrap_owned` is no longer the public wallet unwrap path. It fails closed
  and instructs callers to use `owned_unwrap_sign` plus `owned_unwrap_apply`.
- Added signed/certified `OwnedUnwrapOrder`, validator votes, and
  `OwnedUnwrapCertificate` apply.
- Wallet unwrap is now amount-based: the user enters an amount, the wallet
  selects owned inputs, certified apply credits the exact account amount, and
  any remainder returns as one FastPay change object.
- Python `unwrap_fastpay` and `pftl_transfer.py unwrap-fastpay` use the same
  signed/certified standard unwrap path.
- Raised `MAX_OWNED_INPUTS_PER_TRANSFER` to `2048`, matching Sui-scale
  object-input expectations rather than the earlier prototype cap of `8`.
- Wallet, wallet live feed, and Python owned-object lookup defaults now request
  up to `2048` objects so fragmented wallets are not hidden behind a smaller
  client-side limit.
- Fixed the matching RPC read cap: `owned_objects` now accepts limit `2048`
  while other generic read queries stay at their existing bounded read limit.
- The wallet proxy FastPay broadcast path resolves success at BFT quorum
  (`5/6` on the current devnet), while still broadcasting to all validators.

Redeploy evidence:

- Deployment packet:
  `reports/transaction-improvement/20260629T012710Z-fastpay-owned-objects-read-cap2048-deploy/`
- Deployed node binary SHA:
  `4d124e34fa7549abd1042c1ec20166e125503a9017d4246d5404392afce0a6b0`
- Strict post-deploy preflight:
  `reports/transaction-improvement/20260629T012710Z-fastpay-owned-objects-read-cap2048-deploy/post-deploy-preflight.json`
- Preflight result: green, `6/6` reachable, `6/6` same ledger group, empty
  mempools, and one SSH binary hash group containing all validators.

Verification:

- `cargo test -p postfiat-execution certified_unwrap -- --nocapture`
- `cargo test -p postfiat-execution rejects_oversized_transfer_resource_limit -- --nocapture`
- `cargo test -p postfiat-node read_query_limits_are_bounded -- --nocapture`
- `node wallet-web/src/lib/tx-builder.test.js`
- `npm test` in `wallet-web`
- `npm run build` in `wallet-web`
- `PYTHONPATH=python python3 -m pytest -q python/tests/test_wallet.py`
- `cargo check -p postfiat-node -p postfiat-rpc-sdk -p postfiat-wallet-wasm`
- `wallet-proxy/test_fastpay_quorum.js`

Residual work:

- FastPay send still uses single-input selection in the web wallet. Standard
  unwrap handles multi-input fragmentation; send-side multi-input selection and
  dust consolidation remain follow-up UX work.
- Background object consolidation is not implemented. The wallet should keep
  object details out of the default UX and only surface object-count warnings
  when count affects performance or limits.
