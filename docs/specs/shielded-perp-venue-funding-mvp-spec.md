# Shielded Perp-Venue Funding MVP Spec (Hyperliquid + Lighter), ETH-first

Date: 2026-09-04 (rev 4: implementation-corrected after local MVP build)
Status: design companion. Current code, tests and external gates are tracked in
`shielded-perp-venue-funding-implementation-20260904.md`. Companion to the public proposal
`postfiatorg.github.io/content/research/shielded-hyperliquid-funding-pfusdc.md`.
That document describes the full product; this one specifies the smallest version
that works with primitives already in `postfiatl1v2` and StakeHub, and compares it
against the alternatives a treasury actually has today.

Rev 2 change: the shielded asset is **ETH (WETH vault)**, not USDC. Stablecoins
appear at exactly one place, the Hyperliquid deposit tail, because Bridge2 only
accepts USDC. Everywhere else the flow is ETH end to end. Rationale in §10.

## 0. One-paragraph summary

A treasury deposits WETH into an Ethereum-mainnet WETH vault in standard
denominations, shields the resulting bridge token (call it pfETH) as fixed-size
Asset-Orchard notes, and after a policy delay exits each note through direct
private egress to a fresh PFTL account, burns it, and has a relayer call
`withdrawWithProof` so the vault releases WETH to a fresh Ethereum EOA `E_i`.
`E_i` signs an EIP-7702 delegation to a small `ExitExecutor` contract; a relayer
sponsors one transaction in which `E_i` unwraps and either (Lighter) deposits ETH
into Lighter's L1 contract as margin collateral, or (Hyperliquid) sends ETH
through the native Arbitrum bridge, then on Arbitrum swaps to USDC and transfers
it to Bridge2 in a single sponsored transaction. The outbound Hyperliquid tail
holds USDC only inside that burner transaction and never in a shared address.
Fresh EVM keys need no gas; fresh PFTL accounts receive a small activation amount
from a shared fee sponsor. The build adds route-specific SP1 guests but no new
proof circuit. Marginal cost is flat, roughly $3–10 per exit plus a ~5 bps swap
on the Hyperliquid leg only, versus Railgun's 50 bps of principal. Privacy is
"vault-mixed, standard-denomination, delayed"; it becomes a useful anonymity set
only when unrelated pfETH flow shares the same vault and band.

## 1. What exists today (verified 2026-09-04 against `public/main` @ `3d0e5c01`)

| Primitive | Location | Relevant property |
| --- | --- | --- |
| Ethereum WETH vault | `WETHBridgeVaultL1.sol` | New route-specific contract. It pins WETH, verifier and runtime hashes; stores 9-decimal pfETH atoms with `1 atom = 10^9 WETH wei`; recipients receive WETH |
| Vault release | `withdrawWithProof(publicValues, proof)` | No caller allowlist; recipient and atom amount come from verified route-bound public values; withdrawal commitments prevent replay |
| Existing pfUSDC vault | mainnet Epoch-5 `0x8583409ddbac984ec195dfa06a21103d92403c1e`, verifier `0xa77d5af456ef212303e31727b6ca4888cd771e2c` | Reference deployment. pfETH uses a new verifier instance with WETH, PFETH, route and 9-decimal bindings |
| Vault ingress | `depositV2(amount, pftlRecipient, nonce, routeBinding)` | Public: depositor, amount, PFTL recipient hash |
| Public -> shielded | `asset_orchard_ingress_v1` / `_v2` | Reveals asset and amount; creates one typed note |
| Private swap | `AssetOrchardSwapConservationCircuit` (`asset_orchard_circuit.rs:1024`) | **Whole-note permutation only**; cannot split or merge value |
| Private egress | `asset_orchard_private_egress_v1` | Hides note opening and spend key; reveals `to`, `asset_id`, `amount`, `fee`, nullifier, anchor, timing. Whole note |
| Burn -> Ethereum release | `bridge_out` burn + SP1 finality proof + `withdrawWithProof` | ~3 GPU-minutes per proof; Groth16 verify on Ethereum |
| Vault domains | bridge governance (`shielded_bridge_governance.rs`) | Multiple vault domains already exist (Arbitrum domain was registered, then deprecated 2026-07-25). A WETH domain is a governance registration, not a protocol change. Verify the registration path in §8 |

Consequences:

- **Split/merge is not needed.** Ingress amounts are public in every design, so
  the treasury deposits in denominations and each note is already exit-sized.
- **The vault is already "release any ERC20 to any recipient, relayer-paid".**
  The new on-chain code is one small EIP-7702 delegate that lets a zero-gas EOA
  unwrap and deposit in a sponsored transaction.
- **WETH has no EIP-2612 permit**, so the permit-forwarder design from rev 1 is
  dead for ETH. EIP-7702 (live on Ethereum since Pectra, May 2025) replaces it
  and is strictly more general. Built and deployed as `ExitExecutorV1.sol`
  (§3.4).
- **Amounts are u64 pfETH atoms end to end.** PFETH has 9 decimals and the WETH
  vault enforces `1 atom = 10^9 wei`, so u64 capacity is roughly 18.4 billion
  ETH rather than 18.44 ETH. The `{1, 2, 5, 10}` ETH bands are an anonymity
  policy, not an integer ceiling.
- **The ingress SP1 program is policy-parameterised but route-pinned.**
  `programs/pfusdc-eth-mainnet-ingress` takes `vault_address`, `token_address`,
  `token_runtime_code_hash`, `token_balance_storage_key` as policy inputs, but
  `ROUTE_ID = "ethereum-mainnet-usdc-v1"` is a compile-time constant. A WETH
  route is a copy of the program with a new `ROUTE_ID`, a new reproducible
  build and vkey, and a `VAULT_BRIDGE_ROUTE_ETHEREUM_MAINNET_WETH_V1` constant
  in `core_chain.rs`. WETH9 is not a proxy, so the token storage policy is
  simpler than USDC's (`balanceOf` at slot 3).
- **The finality verifier is immutable-bound to one token.**
  `PFTLFinalityVerifierV1` pins `token`, `tokenRuntimeCodeHash`, and
  `assetIdCommitment` in the constructor (`:113-124`) and checks them on every
  withdrawal (`:278-282`). The WETH vault needs its own verifier instance; same
  code, new constructor args.
- **Route activation uses governed profile activation.** The generated package
  must be submitted through `vault-bridge-route-profile-governance` with the
  exact route-profile hash, SP1 vkey and activation window. A generic
  `bridge_batch_domain` does not activate this proof route.

## 2. Venue facts (verified 2026-09-04)

### Hyperliquid

- Sole canonical deposit rail: Bridge2 on Arbitrum One,
  `0x2Df1c51E09aECF9cacB7bc98cB1742757f163dF7`, native USDC
  `0xaf88d065e77c8cC2239327C5EDb3A432268e5831`. **USDC only.** Minimum 5 USDC.
- Portfolio margin (beta, accounts < $25M): collateral HYPE (LTV 0.65) and BTC
  (LTV 0.5); USDC and USDH borrowable. ETH is not a collateral asset. Both
  collateral assets live on HyperCore, so the account must already be funded.
  BTC could arrive via Unit, but Unit issues per-user deposit addresses, which is
  the CEX labeling problem again. Rejected.
- Conclusion: the Hyperliquid tail must be USDC. Outbound, the design confines
  it to one burner transaction; return USDC remains in a fresh burner until the
  sponsored conversion and can leave per-burner exact-output dust.
- Withdrawal is signed on Hyperliquid only; validators release on Arbitrum in
  ~3–4 minutes; flat 1 USDC fee. The official SDK accepts a `destination`
  distinct from `user`; the StakeHub return action exposes it and requires an
  explicit fresh destination.

### Lighter

- Application-specific zk-rollup on Ethereum. L1 deposit proxy
  `0x3B4D794a66304F130a4Db8F2551B0070dfCf5ca7`. ETH deposits are native.
- **Multi-Asset Margin**, live parameters read from
  `GET /api/v1/assetDetails` on 2026-09-04: ETH `margin_mode=enabled`,
  `loan_to_value 0.70`, `liquidation_threshold 0.85`, `liquidation_factor 0.95`,
  `liquidation_fee 2%`, `global_supply_cap 10,000 ETH`, `user_supply_cap
  2,000 ETH`, `total_supplied 4,263 ETH`, index `$2,451`. XAUT (gold) is also
  margin-enabled at 0.70 with a 500 XAUT user cap; LDO/AAVE/UNI/LINK/LIT/SKY/
  AZTEC are depositable but `margin_mode=disabled`. USDC LTV 1.0.
- L1 deposit verified from Sourcify source of the implementation
  `0x8D692294a4824d868e35B3CEcd734aCf41B2342e`:
  `deposit(address to, uint16 assetIndex, uint8 routeType, uint256 amount)
  payable`, `NATIVE_ASSET_INDEX = 1`, `RouteType.Spot = 1`, `msg.value ==
  amount`, amount a multiple of the tick (`1e10` wei), min `0.001 ETH` at
  genesis config. `registerDeposit` auto-creates an account for an unknown L1
  address, so a burner's first transaction can be the deposit.
- Documented use: deposit ETH, short ETH-perp against it, collect funding. That
  is exactly the position a USD-margin user takes on arrival, so the ETH delta
  is hedgeable inside the venue at zero extra hops.
- Minimums: 5 USDC-equivalent deposit. Accounts resolvable from an L1 address;
  sub-accounts share one L1 address.

## 3. Architecture

```text
[T]  treasury ETH (Ethereum), wrapped to WETH
 |    public, standard denominations, spread over time
 v
[V]  WETH vault (Ethereum mainnet)  ---mint--->  pfETH on PFTL account T'
 |
 v   asset_orchard_ingress (N notes of D each; asset+amount public)
[S]  shielded pool
 |   hold >= policy delay; notes are indistinguishable from any other D-note
 v   asset_orchard_private_egress_v1  ->  fresh PFTL account P_i  (amount D public)
[B]  bridge_out burn from P_i, recipient = fresh Ethereum EOA E_i
 |   SP1 finality proof; relayer calls vault.withdrawWithProof
 v
[E]  E_i holds D WETH on Ethereum, zero ETH
 |   E_i signs 7702 delegation to ExitExecutor + signed call batch;
 |   relayer submits one type-4 transaction and pays gas
 |
 |-- Lighter:      unwrap D -> Lighter.depositETH{value: D}(E_i)
 |                 -> Lighter account credited, flagged as margin asset
 |                 (zero stables, zero bridges, two contracts)
 |
 '-- Hyperliquid:  unwrap D -> ArbInbox.depositEth{value: D}   (native bridge)
                   ~10–15 min; E_i now holds D ETH on Arbitrum (same address)
                   E_i signs 7702 batch on Arbitrum; relayer submits:
                     swap ETH -> USDC (Uniswap v3 ETH/USDC 5 bps pool)
                     USDC.transfer(Bridge2, usd)
                   -> Hyperliquid account E_i credited
                   outbound USDC lifetime: one transaction, one burner
```

### 3.1 Denomination policy

- Bands in ETH units: `D ∈ {1, 2, 5, 10}` ETH (≈ $2.4k–$24k at $2,440/ETH;
  policy bands, §1). A $100k account is four or five notes across buckets.
  Bands are fixed in ETH, never in USD, so USD drift never fingerprints a note.
- No band is offered until the vault has released at least `k_min` exits of
  that size to unrelated recipients in the trailing window (`k_min` and window
  governed; initial suggestion 8 / 30 days).
- Ingress amounts are exact multiples of a band. Fees on PFTL are paid from a
  separate public fee account, never deducted from the note.
- ETH delta during the delay window is the user's. Two mitigations, both
  outside the shielded flow: hedge with a short on any already-public account
  for the delay period, or accept it. A pfUSDC variant (rev 1 of this spec)
  remains available for users who refuse delta; it re-imports every stablecoin
  risk in §10.

### 3.2 Planner and scheduler (wallet code, StakeHub or CLI)

Inputs: target allocations `{venue, amount_eth}`, policy. Outputs: an ingress
plan and an exit schedule.

- Ingress plan: `n_D` notes per band, each ingress its own action, spaced by a
  randomized interval drawn from the same distribution the scheduler uses for
  exits.
- Exit schedule: `release_at = ingress_at + delay_min + U(0, jitter)`, quantized
  to a release bucket (e.g. 1 hour). Exits from one batch to different venues
  or accounts must not share a bucket unless the bucket also contains unrelated
  exits.
- Fresh keys: `(P_i, E_i)` are derived from a protected root with a per-account
  index and never reused. A shared PFTL fee sponsor activates `P_i` with the
  minimum public fee reserve; this creates a public sponsor-to-burner edge but
  no treasury edge. EVM transactions are relayer-paid. On the Hyperliquid path
  `E_i` is also the Arbitrum and Hyperliquid account for the outbound lifetime.
- The planner refuses to schedule an exit whose band has fewer than `k_min`
  unrelated candidates and reports the downgraded label instead.
- The planner is the only place the mapping `note -> P_i -> E_i` exists;
  encrypted at rest under a key separate from the spend key; never written to
  the StakeHub receipt store.

### 3.3 Private egress and burn

- `asset_orchard_private_egress_v1` with `to = P_i`, `amount = D`. `P_i` is a
  fresh PFTL account whose public history includes shared-sponsor activation,
  one inbound private egress and one `bridge_out` burn.
- `bridge_out` burn packet recipient = `E_i`, amount = `D`.
- Proof generation and `withdrawWithProof` submission are performed by a relayer
  identity common to all users of the vault. There is no perp-specific artifact
  on Ethereum; a Lighter exit and a plain pfETH redemption look identical until
  the next transaction.

### 3.4 `ExitExecutor` (new contract, Ethereum mainnet + Arbitrum, ~50 lines)

An EIP-7702 delegate. `E_i` signs an authorization pointing its code at this
contract, plus an EIP-712 signature over the call batch. Any relayer submits the
type-4 transaction and pays gas.

```solidity
interface IExitExecutor {
    struct Call { address to; uint256 value; bytes data; }
    /// Executes `calls` from the delegated EOA. `sig` is the EOA's EIP-712
    /// signature over (chainId, address(this), nonce, keccak(calls), deadline).
    /// Replay-protected by `nonce`; anyone may submit; the EOA holds no gas.
    function execute(Call[] calldata calls, uint256 nonce, uint256 deadline,
                     bytes calldata sig) external;
}
```

- Built: `crates/ethereum-contracts/src/ExitExecutorV1.sol`, 12 Foundry
  tests including a cross-implementation EIP-712 digest fixture reproduced by
  the StakeHub signer (`tests/test_shielded_exit.py`). Nonce lives at an
  ERC-7201 slot; signatures bind chain id, EOA, and implementation address.
- Deployed 2026-09-04 through the StakeHub launch-session path (bytecode hash
  pinned): Ethereum mainnet `0x0c51DB40B16691319E027Ae7E1fb71A8D4F2b8bA`
  (677,363 gas), Arbitrum One `0xf56aB47F7D720c96145E1404E2630954F6b2F16A`.
  Unreviewed; redeploy after review, the EOA delegation is one signature.
- No admin, no upgradeability, no storage beyond the nonce. The EOA's
  delegation can be revoked by a later 7702 authorization.
- Lighter batch: `[WETH.withdraw(D), Lighter.depositETH{value:D}]`.
- Hyperliquid batch, Ethereum: `[WETH.withdraw(D), ArbInbox.depositEth{value:D}]`.
  Arbitrum: `[SwapRouter.multicall(exactOutputSingle, refundETH),
  USDC.transfer(Bridge2, usd)]`. The exact-output maximum and deadline are signed
  into the batch.
- Fallback if 7702 is unavailable on a chain: relayer sends a fixed gas dust
  (e.g. 0.003 ETH) to every recipient as part of the standard release service.
  Uniform across all exits, so it adds no distinguishing signal, but it costs
  the relayer an extra transaction and leaves dust in `E_i`.
- Across (ETH, Ethereum -> Arbitrum) is a faster alternative to the native
  bridge for the Hyperliquid leg at a few bps; off by default.

### 3.5 Venue deposit

- Lighter: native ETH enters the L1 contract from `E_i`; the account is created
  or resolved by L1 address. The official SDK action enables unified trading
  and ETH margin from an API key derived for the same owner account.
- Hyperliquid: a plain USDC transfer to Bridge2 from `E_i` on Arbitrum after
  the swap credits the Hyperliquid account `E_i`. Minimum 5 USDC; output is rounded to
  the band's USD floor and the remainder stays as ETH in `E_i` for future gas.

### 3.6 Return flow

```text
Lighter:     withdraw ETH (band D) to the owning E_i on Ethereum
             -> E_i wraps and deposits into the WETH vault
Hyperliquid: withdraw USDC (band) to fresh C_i on Arbitrum
             -> sponsored 7702 batch swaps exact-output ETH and calls
                ArbSys.withdrawEth to the same C_i on Ethereum
             -> wait through the Nitro challenge period and finalize the outbox
             -> C_i wraps and deposits into the WETH vault
Both:        pfETH -> shielded -> delayed private consolidation -> treasury
```

- Withdraw only in bands; leave venue dust in the venue account.
- Lighter binds withdrawals to the owning L1 address, so its return reuses
  `E_i`; Hyperliquid supports a fresh arbitrary destination `C_i`.
- Hyperliquid USDC sits in `C_i` from venue withdrawal until the sponsored
  return batch; exact-output conversion can leave per-burner USDC dust.
- Arbitrum vault ingress is never used. The Nitro L2-to-L1 challenge and
  permissionless outbox finalization apply to the Hyperliquid return.

## 4. Privacy claim

Labels: StakeHub vocabulary plus one new label.

| Label | Where it applies |
| --- | --- |
| `public` | treasury deposit, vault ingress, every Ethereum/Arbitrum transfer, venue activity |
| `private middle` | note ownership and lineage inside the pool |
| `private egress` | the exit proof (note opening hidden; `to`, amount, timing public) |
| `vault-mixed exit` | an exit whose (band, bucket) contains `>= k_min` unrelated exits |

Claim: a `vault-mixed` exit removes a direct treasury-to-burner edge and gives
an observer the set of plausible same-band, policy-window matchings measured by
the observer tool. Amount, timing, a thin vault and the public shared-sponsor
activation of `P_i` can shrink that set. With only treasury flow in the vault,
the candidate set is still the treasury's accounts; meaningful anonymity needs
unrelated vault users and deliberately uncorrelated timing.

Not hidden: aggregate treasury deposits, aggregate vault releases, venue
activity under any known address, behavioral fingerprints, anything the
relayer, RPC operator, venue, sequencer, or planner host can see.

ETH-specific: the 7702 delegate address is public and shared by every exit, so
"delegated to ExitExecutor" is the same signal as "released by the vault";
neither narrows the candidate set below the band/bucket bound.

## 5. Cost model

Per exit, Ethereum gas ~0.5 gwei and ETH ~$2,440 (2026-09-04), Arbitrum gas
~0.02 gwei. Foundry measured the sponsored unwrap+deposit batch at 106k gas.

| Item | Lighter | Hyperliquid |
| --- | ---: | ---: |
| PFTL ingress + private egress + burn | ~$0 | ~$0 |
| SP1 finality proof (~3 GPU-min) | $0.10–0.50 | $0.10–0.50 |
| `withdrawWithProof` (Groth16 verify + transfer, ~350k gas) | $1–3 | $1–3 |
| Sponsored 7702 batch on Ethereum (unwrap + deposit, ~110–200k gas) | $0.2–1 | $0.2–1 |
| Arbitrum: bridge claim + swap + Bridge2 deposit | – | $0.10–0.30 |
| ETH -> USDC swap fee + impact (5 bps pool) | – | ~5 bps of D |
| Relayer premium (governed, flat) | $1–5 | $1–5 |
| **Total** | **~$3–10, flat** | **~$3–10 flat + ~5 bps** |

At $100k the Hyperliquid leg costs ~$50 in swap fees; Railgun costs $500 for
the same round trip half. Lighter carries no percentage cost at all. At 10 ETH
(~$24k) the flat cost is ~$3–8 against Railgun's ~$122.

Costs the MVP does not carry: no float, no percentage protocol fee, no
liquidity-pool slippage on the shielded leg, no CCTP dependency.

Costs it does carry: delay (policy-bounded, hours); ~20 minutes of latency on
the Lighter leg (proof + Ethereum finality) and ~35 minutes on the Hyperliquid
leg (plus native bridge); ETH price exposure for the delay window.

## 6. Comparison

Scenario: fund 10 fresh venue accounts with $100k each ($1M total) from a known
treasury, then recycle $1M back after 60 days.

| | Direct funding | CEX hop | Railgun on Arbitrum | pfETH MVP (this spec) |
| --- | --- | --- | --- | --- |
| Out-and-back cost on $1M | ~$5 gas | ~$0–20 fees | $10,000 (25 bps in, 25 bps out) + gas + broadcaster | Lighter ~$60–200 flat; Hyperliquid ~$60–200 + ~$1,000 swap |
| Cost at $10M | ~$5 | ~$0–20 | $100,000 | ~$60–200 (+$10k swap on HL only) |
| Treasury -> account link on chain | deterministic | deposit side labeled forever (persistent per-user deposit address); withdrawal side broken (omnibus hot wallet) | broken | broken |
| Who holds the map | everyone | the exchange (KYC identity, permanent, subpoenable) | nobody on chain; the wallet | the planner host; nobody on chain |
| Amount/timing correlation | trivial | strong: same amount in and out minutes apart | user's responsibility | policy-enforced bands, buckets, delay |
| Gas on fresh keys | treasury-funded (clustering signal) | CEX withdraws to fresh address | broadcaster network | never: 7702 sponsored on every hop |
| Anonymity set | none | all users of that exchange | all Railgun shielders in the window | all vault exits in band/bucket; small until external flow |
| **Stablecoin exposure** | full (if USDC) | full, plus exchange | full: Railgun holds the token you shield | **zero on Lighter; outbound one burner transaction on Hyperliquid; return per-burner until conversion** |
| **Issuer freeze surface** | your address | exchange omnibus | Railgun pool contract holds USDC for everyone | **none on the pool; per-burner on HL, uncorrelated** |
| Programmable from CLI for N accounts | yes | no | SDK, per-wallet | yes |
| Custody | self | custodial during hop | self | self; vault is proof-gated |
| Compliance posture | n/a | structuring-shaped pattern at the exchange | POI-gated | selective disclosure artifacts |
| Build required | none | none | none | WETH vault deploy + domain registration, planner, `ExitExecutor`, 2 relayers |

Reading the table:

- **Direct funding** is free and useless; it is the problem statement.
- **CEX hop** is the real incumbent. Cheap, breaks the on-chain link, but puts
  the full mapping in a custodian's records under a KYC identity, leaks through
  amount and timing, and is manual with withdrawal limits and review.
- **Railgun** is the only non-custodial competitor with a live anonymity set.
  50 bps per round trip is $10k per $1M and scales linearly. Its USDC pool is
  also one address Circle can freeze under a GENIUS Act order, and everyone
  inside it is frozen together.
- **pfETH MVP** matches Railgun's on-chain property at roughly 1/100th the cost
  on Lighter and 1/10th on Hyperliquid, adds denomination and delay policy,
  keeps every hop gasless for fresh keys, and holds no stablecoin in any
  shared address. Its weakness is the anonymity set, which the price advantage
  is meant to buy.

Break-even against Railgun: ~50 bps of routed principal saved. At ~6
engineer-weeks plus one contract review, it pays for itself at roughly $2–5M
of routed volume.

## 7. Delivery

| Step | Owner surface | Exit criterion |
| --- | --- | --- |
| D1 Denomination + scheduler policy | **done**: `StakeHub/stakehub/shielded_exit_planner.py` (bands, buckets, sibling rule, k_min gate, HKDF keys, AES-GCM map) | policy set: `k_min=8`, delay 6–24h, bucket 1h, bands {1,2,5,10} ETH |
| D2 WETH vault + route | **local complete**: WETH vault, pfETH guest, deterministic package and route profile; **external gates**: review, deployment, PFETH bootstrap and validator activation | fresh accepted receipts, active route readback and one 0.01 ETH round trip |
| D3 Fresh-account egress | **local complete**: PFTL materializer, shared-sponsor activation and relayer job; **external gate**: live pfETH | `P_i` activation receipt, private egress, burn to `E_i`, proof release and exact WETH postcondition |
| D4 `ExitExecutor` | **done**: `ExitExecutorV1.sol`, 12/12 tests, deployed mainnet + Arbitrum One | live sponsored unwrap+deposit from a zero-ETH EOA (probe ready, §8) |
| D5 Lighter deposit | batch builder and live deposit probe **done**; official-SDK margin action **local complete** | live margin enable remains a funded-account external gate |
| D6 Hyperliquid leg | batch builders and live 5 USDC deposit probe **done**; arbitrary-destination withdrawal **local complete** | full pfETH-funded live leg remains an external gate |
| D7 Return flow | immutable Lighter and Hyperliquid return jobs **local complete** | funded venue withdrawals, Nitro finalization and vault ingress remain external gates |
| D8 Observer test | incremental observer, release gate and offline matching enumerator **local complete** | run against the deployed-vault cohort after live activation |

Order: D1 -> D2 -> D3 (proves the vault leg) -> D4 -> D5 (simplest venue first)
-> D6 -> D7 -> D8.

## 8. Open items to verify before build

1. ~~Lighter ETH margin parameters~~ resolved (§2): LTV 0.70, user cap 2,000
   ETH, global cap 10,000 ETH with 4,263 supplied. Every band fits.
2. ~~Lighter L1 deposit and margin action~~ resolved: native ETH auto-creates
   the account; StakeHub uses the official SDK with the burner-derived API key
   to enable unified trading and ETH margin. Live mutation remains manager-gated.
3. EIP-7702 on Arbitrum One: `ArbSys.arbOSVersion()` returns 116 (ArbOS 61,
   past the ArbOS 40 7702 activation). Live type-4 probe written and ready:
   `StakeHub/scripts/shielded_exit_probe_arbitrum.py` (swap 5 USDC exact-out,
   refund, Bridge2 transfer, HL `clearinghouseState` check). Blocked only on
   funding the probe relayer `0x62EF50a792641Cd1a064fD0B1Da933b14Fd0D6c9`
   with ~0.003 ETH on Arbitrum; the running StakeHub agent build
   (`StakeHub-master-e6`, Aug 11) only sends native ETH to the global
   whitelist. Same relayer needs ~0.005 ETH on mainnet for the Lighter probe.
4. WETH vault: local package is complete with `WETHBridgeVaultL1`, a new
   `PFTLFinalityVerifierV1`, WETH ingress guest/vkey, PFETH bootstrap operations
   and a governed route profile. External review, a fresh checkpoint, deployment
   and `vault-bridge-route-profile-governance` activation remain open.
   `paused` owner = StakeHub wallet `0x1455…` at deploy.
5. Uniswap v3 ETH/USDC depth on Arbitrum at band sizes; whether a 1 bp pool
   exists with usable depth; slippage bound to sign into the batch.
6. ~~Hyperliquid arbitrary withdrawal destination~~ resolved in the official
   SDK and exposed by StakeHub; a live funded withdrawal remains external.
7. ~~Fresh PFTL burner activation~~ resolved with a shared fee sponsor and an
   exact finalized accepted-receipt gate; live activation remains external.
8. Mainnet vault operating caps, pause state and active route readback must be
   verified immediately before any live round trip.

## 9. What phase 2 adds, and when

Build only after the MVP has routed real volume and at least one external party
uses the vault:

- Split/merge gate in `AssetOrchardSwapConservationCircuit` so odd amounts can
  be shaped privately.
- One SP1 proof covering N burns, so `withdrawWithProof` amortizes across a
  bucket.
- Governed `k_min` enforcement at the protocol rather than the wallet.
- Selective-disclosure artifacts for enumerated accounts and periods.
- If Hyperliquid adds ETH as portfolio-margin collateral, delete the swap and
  the Hyperliquid leg becomes identical to Lighter's: zero stables.

## 10. Why ETH, and where stables remain

Stablecoins are the only asset class in this flow with a third party who is
legally obliged to freeze on order. Under the GENIUS Act (July 2025) and
Treasury's April 2026 proposed rules, permitted issuers must be able to freeze,
seize, or burn on lawful order, including in the secondary market. A USDC
shielding pool is therefore one address holding every user's principal, where
one order freezes everyone. Railgun's USDC pool has exactly this shape. ETH has
no issuer, no freeze function, and no secondary-market obligation.

Second, ETH is gas. The rev 1 design spent most of its new code on moving USDC
between chains without any fresh key holding ETH. With ETH as the asset that
problem disappears; one sponsored transaction unwraps and deposits.

Third, the legal framing improves. An ETH-in, ETH-out pool with public ingress,
standard denominations, and delays is the same object as Tornado Cash and
Railgun's ETH pools, where *Van Loon* (5th Cir. 2024) held immutable contracts
are not sanctionable property and OFAC delisted. A stablecoin rail into perp
venues has no such precedent.

Where stables remain, and why each is tolerated:

| Place | Dwell | Who can act | Why tolerated |
| --- | --- | --- | --- |
| Hyperliquid deposit tail | one Arbitrum transaction in burner `E_i` | Circle, per address, after the fact | Bridge2 accepts nothing else; per-burner, uncorrelated, no shared address to freeze |
| Hyperliquid account balance | duration of the position | Hyperliquid (not Circle; balance is internal) | inherent to the venue; identical under every alternative |
| Hyperliquid return leg | from venue withdrawal until the sponsored return batch in burner `C_i`; possible exact-output dust | Circle, per address | per-burner rather than shared; swapped to ETH before the Nitro exit |
| Lighter | none | – | ETH is native margin collateral |

Everything else in the flow (vault, pool, egress, bridges, relayers) never
touches a stablecoin.

## Sources

- `crates/ethereum-contracts/src/ERC20BridgeVaultL1.sol` (`immutable token`
  `:74`, `withdrawWithProof` `:191`, `token.transfer` `:216`);
  `ERC20BridgeVaultV2.sol:4` (`IERC20BridgeTokenV2` interface).
- `crates/privacy_orchard/src/asset_orchard_circuit.rs:1024` (permutation
  gate), `crates/privacy_orchard/src/verify.rs:1459` (same-asset reject).
- `docs/status/A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md` (mainnet ids).
- `docs/business/pfusdc-arc-grant-proposal-20260828-v3.md` (proof timing,
  Arbitrum deprecation).
- StakeHub `docs/current-sprint/end-to-end-shielding-privacy-requirement.md`.
- Hyperliquid docs: portfolio margin (HYPE LTV 0.65, BTC 0.5, supply caps,
  <$25M beta); USDC/Bridge2 (5 USDC minimum, withdrawal flow, 1 USDC fee).
- Lighter docs: multi-asset margin (TAV formula, LTV/LT/LF parameters,
  per-user and global caps, UTA requirement); API deposits doc (L1 contract
  `0x3B4D…5ca7`, ETH vs ERC20 handling); The Defiant 2026-04-24 (ETH first
  margin asset); exchange-compare 2026-06 (ETH still the only non-USDC asset).
- Railgun docs: 0.25% shield, 0.25% unshield, broadcaster premium on gas only.
- GENIUS Act (2025-07-18) and Treasury NPRM (April 2026): issuer freeze/seize/
  burn obligations. *Van Loon v. Treasury*, 5th Cir. 2024; OFAC delisting
  2025-03-21.
- EIP-7702 (Pectra, Ethereum mainnet 2025-05-07).
