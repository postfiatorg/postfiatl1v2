# a666 v2 Mainnet Primary-Market Deployment

> **Historical architecture notice (2026-08-01):** This records the original
> A666 deployment lineage, which used an internal StakeHub-operated proof
> profile. It is retained for deployment and evidence integrity. It is not the
> current public reserve-proof architecture and must not be used to introduce
> a StakeHub dependency into a wallet, public proxy, validator, signer, or new
> NAVCoin. A provider-neutral A666 successor requires the governed migration
> in the
> [deferred StakeHub decoupling plan](../deferred-plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md).

**Status:** executed through empty-pool initialization; proof-backed export,
seeding, and public activation remain pending
**Canonical spec:** `docs/plans/A666-END-TO-END-MAINNET-PRIMARY-ISSUANCE-SPEC-20260727.md`

This runbook records the production a666 v2 launch sequence. The native asset,
historical internal-operator NAV profile, paused Ethereum stack, and empty hookless Uniswap pool were
deployed on 2026-07-27 under explicit production-owner authorization. It does
not authorize further customer-principal movement, redemption execution, or LP
funding. Omit `--broadcast` for the remaining steps until the production owner
has approved the frozen manifest and exact transaction package.

## 1. Required frozen inputs

Copy `crates/ethereum-contracts/script/a666-mainnet.env.example` into a
secret-manager-backed environment. Do not put a funded private key in a file.
Freeze and independently review:

- the source commit and clean/dirty worktree declaration;
- a666 v2 asset ID (`max_supply=None`, precision 6);
- route, policy, reserve-packet, chain/genesis, and initial-checkpoint values;
- the SP1 guest ELF hash and program vkey;
- the official SP1 verifier, Uniswap v4 PoolManager, PositionManager, Permit2,
  and mainnet USDC addresses and runtime code hashes;
- the predicted wA666, receipt-verifier, and controller addresses;
- the controller runtime code hash for the exact constructor tuple;
- the predicted wA666/USDC PoolKey and PoolId;
- governance, position owner, opening reserve, LP funding authority, and the
  policy for custody/reallocation of subscription-funded NAV reserve principal.

The PFTL route stays disabled until deployment read-back and proof gates pass.

## 2. Build and test

From the repository root:

```bash
cargo test --workspace
(
  cd programs/pftl-uniswap-receipt
  cargo check --locked --no-default-features
  PATH="$HOME/.sp1/bin:$PATH" cargo prove build --locked --output-directory elf
)
(
  cd crates/ethereum-contracts
  forge build
  forge test
)
```

Hash the resulting guest ELF, derive the SP1 program vkey with the campaign's
pinned SP1 6.3.1 toolchain, and record both in the manifest. A Rust unit test,
mock verifier, or guest `cargo check` does not satisfy the genuine-proof gate.
The current derived candidate is recorded in
`programs/pftl-uniswap-receipt/program-info.json`; rebuild it after any source
or lockfile change and reject any hash drift.

Build the host executor/prover separately because its SP1 SDK dependency is
intentionally excluded from the L1 workspace. A compatible `protoc` must be
installed; set `PROTOC` to its absolute path when it is not on `PATH`:

```bash
cargo build --release --manifest-path tools/pftl-uniswap-prover/Cargo.toml
tools/pftl-uniswap-prover/target/release/pftl-uniswap-prover program-info
```

Use `receipt --witness FILE --output-dir DIR` or
`checkpoint --witness FILE --output-dir DIR` for native/SP1 execution. Add
`--prove` only in a release build with the campaign's reviewed prover
configuration. The checkpoint witness is receipt-independent and is the
stale-checkpoint liveness path when no user export exists.

## 3. Pinned mainnet-fork rehearsal

Choose a reviewed mainnet block and start an Anvil fork. Re-read every external
component's code hash at that block. Export the frozen variables, including an
unfunded rehearsal key, then run:

```bash
cd crates/ethereum-contracts
forge script script/DeployA666PrimaryMarket.s.sol:DeployA666PrimaryMarket \
  --rpc-url "$A666_FORK_RPC_URL" \
  -vvvv
```

This first invocation deliberately omits `--broadcast`. Review the simulation,
predicted addresses, constructor inputs, gas, token controller lock, token
ownership, verifier bindings, and emitted deployment record.

The first disposable-fork derivation may set
`A666_CONTROLLER_RUNTIME_CODE_HASH=0x00...00`; the script emits the actual
runtime hash. Freeze that nonzero hash, rerun the simulation, and require the
script's equality check for every reviewed fork and production package.

After review, broadcast only to the disposable fork:

```bash
forge script script/DeployA666PrimaryMarket.s.sol:DeployA666PrimaryMarket \
  --rpc-url "$A666_FORK_RPC_URL" \
  --broadcast \
  -vvvv
```

Read back:

```text
wA666 decimals = 6
wA666 controller = deployed primary-market controller
wA666 controllerLocked = true
wA666 owner = frozen governance address
controller destination chain = 1
route cap = 2,000,000e6
packet cap = 250,000e6
verifier chain/route-id/assets/controller/token/vkey/checkpoint = frozen manifest
controller runtime code hash = frozen manifest and deployment read-back
new primary mint pause = enabled until route activation
return burn = callable while minting is paused
```

Run a genuine SP1 proof through the deployed verifier and controller. Exercise
the 100,000-a666 issue/export/consume path, then the 1,000,000-a666 capacity
case as four packets of 250,000 a666. Also exercise return, redemption,
deadline cancellation/refund, and all replay negatives.

## 4. Canonical seed and pool rehearsal

Opening wA666 must come from the proof-backed PFTL opening issue and verified
export to `A651ToA666MigrationV1`. Seed wA666 may leave that contract only
after the same transaction burns the corresponding operator-held a651. Never
use an owner mint, a direct a651 transfer, or successor inventory reserved for
PoolManager/external legacy holders.

Set `A666_WRAPPED_TOKEN` to the deployed fork token. Ensure the token ordering,
equal 6-decimal seed amounts, fee `500`, tick spacing `10`, no hooks, Q96
price, PoolId, ticks, and maximum token spends are independently recomputed.
Then simulate without broadcast:

```bash
forge script script/InitializeA666UniswapV4.s.sol:InitializeA666UniswapV4 \
  --rpc-url "$A666_FORK_RPC_URL" \
  -vvvv
```

Only after reviewing approvals, amounts, position owner, price, PoolId, and gas
may the operator use `--broadcast` on the disposable fork. The script performs
pool initialization and LP creation directly from the EOA and deploys no
temporary helper contract.

## 5. Production no-broadcast package

Regenerate both script simulations against a fresh pinned mainnet read and
archive:

```text
source/lockfile/ELF hashes
SP1 vkey and genuine proof receipt
constructor tuple and predicted addresses
creation/runtime bytecode hashes
PoolKey, PoolId, Q96 price, ticks, and seed amounts
unsigned transaction data, nonces, gas estimates, and ETH ceiling
PFTL asset/policy/route payloads and six-validator read-back plan
all positive, negative, replay, restart, and conservation reports
```

No-broadcast output is not a deployment. Production broadcasts require
separate authorization for deployment gas, opening reserves, LP capital, and
any real-value user test. There is no separate 2,000,000-pfUSDC redemption
funding transaction: primary subscriptions supply the NAV reserve principal
used by primary redemption.

## 6. Production transaction order and rollback boundary

After explicit authorization:

1. deploy wA666, receipt verifier, and primary-market controller;
2. lock the token controller and transfer token ownership to governance;
3. verify all Ethereum code and immutable bindings;
4. register a666 v2 and its historically deployed SP1 profile, finalize the fresh NAV
   packet, mint the exact proof-backed opening supply, and initialize the
   disabled route with that supply as opening inventory;
5. verify identical PFTL state/route digests on all validators;
6. export the opening inventory to the immutable migration contract, authorize
   that contract on the legacy a651 supply controller, and migrate the
   operator allocation by burning a651 before wA666 release;
7. run the smallest new-cash issue/export/return/redemption cycle and prove
   that redemption releases that same subscription-funded principal;
8. use migrated operator wA666 for the canonical pool seed;
9. initialize and seed wA666/USDC;
10. run monitoring and conservation read-backs;
11. enable new primary issuance only after every launch gate passes.

Once any immutable Ethereum contract is broadcast, rollback means abandoning
that deployment and using a new frozen tuple/address lineage. Never repair an
incorrect deployment by relabeling its trust class or transferring controller
authority. Pausing new mint must preserve burn, return import, and conforming
redemption from NAV reserve custody. Use the signed
`pftl_uniswap_route_pause` PFTL transaction for an emergency inbound halt;
resume fails unless live value, current NAV policy, and the NAV reserve
accounting/liquidity checks are all valid.
