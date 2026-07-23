# NAZGUL HANDOFF — a666 / wA666 controlled bridge build

Date: 2026-07-23

Overall status: **PARTIAL — controlled fork passed; live ce22 route and live
primary/export remain blocked and were not falsely claimed.**

Trust class everywhere in the staged route: `CONTROLLED`.
Public routing: disabled.
Ethereum mainnet spend/deployment/pool/liquidity: none.
Legacy `a651`: untouched.
Founder `$5` job: untouched.

## Completed

### Live ce22 additive issuance

- Six validators remained converged with an empty mempool.
- Fresh asset:
  `300bf48a63a94770b6e67817f88cd1abf77e7f592a061e15682d7fd9973260af4c2e631e32df3c2c402b7d2fe272a293`
- Asset code/precision: `a666` / `6`.
- Issuer: `pffcb93d9f87a843a8aa34e1adf241f5d58143e81b`.
- Reserve operator:
  `pfd0c86d9084915e1fefd22eab891806397d5a5937`.
- Holder trustline:
  `pfab9b9228942e5c529633a13aa271d5297bec6353`.
- Finalized epoch `1`, NAV `1_000_000`, circulation `0`, verified net assets
  `0`. The zero seed avoids double-counting legacy `a651` reserves.
- The six additive operations finalized at heights `291` through `296`; the
  four remote synchronous rounds each carried five votes and verified every
  send.
- Current read-only check: `6/6`, height `296`, empty mempools, tip
  `7bea52c025b519ed3c1f60cf9c3afd1fa11416b063b1dd40998ec9ef655da514c084790a2d45a1a9a656e4eca500bf22`,
  state root
  `ae09bfefa1b870c3aacda61913c850d836395b3e6bb74c00bf62e5c28445634a8b267cd74b896f228af625ecbf418296`.

Evidence: `live-ce22/`.

### Controlled Ethereum mainnet-fork end-to-end

- Fresh deterministic instances of wrapped token, controlled verifier,
  replay registry, controller, v4 router, adapter, and helper were deployed on
  a fresh fork only.
- Official Uniswap v4 mainnet contracts had bytecode at fork block `25598324`.
- Pool:
  `0xb992b3761ff7038efe6ee4c17e7212503c44c6ef8c3e7915b654645f9470b3cf`.
- Seed `100_000` wA666 atoms came from the staged primary-subscription plus
  export packet, not a direct EVM mint.
- External buy: `10_000` USDC atoms in, `9_086` wA666 atoms out.
- External sell: `4_543` wA666 atoms in, `5_231` USDC atoms out.
- Wrapped supply was `100_000` before and after the external AMM trades.
- Mint-and-swap preflight and execution both returned `9` USDC atoms.
- Packet and source receipt were both consumed; replay/consume-once rejection
  is covered by the controller suite.
- Route digest:
  `ae558831a5a0c80bd8788c2289ea70ff10ba49a72274b8df026fea8855e27c59d5b612091f9efa3a8d1db9b5e4c197dc`.
- Launch digest:
  `25bbbdf0d52ee0781a38f39873b3353668b09f13de8debab5abafac34b8eeb4a321e97dc9263659801bfc2e3083d57d0`.

This is fork evidence, not a persistent deployment. Its addresses disappeared
when the fork stopped and must not be registered as a durable live route.

Evidence: `controlled-mainnet-fork-rerun/`.

### Consensus and wallet completion

- Added strict support for internal `CONTROLLED` consensus routes while keeping
  `live_value_enabled=false`.
- A controlled route cannot attach a checkpoint policy or enable public
  live-value routing.
- Existing `BFT_CHECKPOINT` policy, committee, checkpoint, receipt-log, and
  event-binding verification remains unchanged.
- Controlled destination consume, refund, and return are explicitly
  issuer/reserve-operator attested; consensus still enforces finality-depth
  arithmetic, caps, packet state, replay protection, and the section 7.4 supply
  invariant.
- Route status now reports `movement_model=burn_mint`, `refund_model`,
  `live_value_enabled`, `route_live`, and the machine-readable trust class.
- Wallet packets expose the three labels:
  `Primary issuance — creates a666 supply finalized NAV`,
  `Bridge export → wA666 — moves issued a666 to Ethereum`, and
  `Uniswap trade — existing wA666 AMM`.

Release binary built but not deployed:

```text
target/release/postfiat-node
sha256 76072718505b275f80c6550667f75141e78fe0a42a0b4bfbbf61a7884a48c978
```

## Verification

- Strict controlled + BFT consensus end-to-end test: `1 passed`.
- Node route/status test: `1 passed`.
- RPC response validation test: `1 passed`.
- Fuzz harness compile: passed.
- Ethereum controller Foundry suite: `36 passed`.
- Wallet proxy regression suite: `24 passed`.

See `VERIFICATION.md`.

## Not completed / blockers

1. **No live ce22 consensus route entry.** The currently deployed ce22 binary
   admits only `BFT_CHECKPOINT`; relabeling this controlled route would be a
   false trust claim. The new binary fixes this with a fail-closed staging gate,
   but was not rolled out because orc2's
   `eth-l1-fast-lane-p0-20260723` session is active on the shared operational
   surface.
2. **No persistent staging Ethereum deployment.** The successful fork was
   intentionally terminated. Registering its ephemeral addresses on ce22 would
   create a dead route. Sepolia RPC/funding was not present.
3. **No authorized live primary subscription.** The designated holder has zero
   pfUSDC, and the founder `$5` funding job was explicitly out of scope.
4. Therefore no claim is made that live ce22 executed
   `pfUSDC -> a666 -> export`. The completed full path used the deterministic
   controlled sidecar plus a real mainnet-fork EVM/Uniswap execution bound to
   the live asset id.

The attempted validator-0 sidecar route file was quarantined inert at:

```text
/var/lib/postfiat/validator-0/a666-inert-sidecar-attempt-20260723/
```

Consensus route count remains zero. No restart or reset occurred.

## Safe continuation order

1. Wait for nazgul/orc2 clearance of the shared fleet surface.
2. Rebuild from the reviewed commit and verify the binary hash.
3. Roll one validator at a time, proving `5/6` or better before each next
   validator; abort on root/domain divergence.
4. Deploy a persistent `CONTROLLED` stack on Sepolia or start a retained,
   named fork; bind its final addresses into a new route/launch digest.
5. Initialize the ce22 route with `live_value_enabled=false`; confirm six-node
   route-status equality.
6. Obtain explicitly authorized non-founder-job pfUSDC staging funding.
7. Execute live primary subscription and export, consume the exact packet on
   the persistent staging EVM, mark controlled destination consume on PFTL,
   seed the new pool, buy, sell, and reconcile every bucket.

ETA after both fleet clearance and authorized staging pfUSDC/persistent EVM
access: **6–10 hours** to a durable controlled end-to-end packet. Gate 5 and
mainnet remain separate, uncleared milestones.
