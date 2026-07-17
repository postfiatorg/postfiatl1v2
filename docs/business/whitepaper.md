# PostFiat Business Whitepaper

*Status: non-normative commercial working draft. This is not a protocol
whitepaper and must not be used to infer implemented consensus, security,
privacy, bridge, custody, or recovery guarantees. The sole canonical protocol
candidate is [../whitepaper.md](../whitepaper.md), subject to its current
implementation boundaries and `SECURITY.md`.*

## Thesis

PostFiat is a settlement chain for private financial claims that need public
enforcement. The core product is not a generic data feed. It is a ledger where
reserve packets, NAV claims, disclosure receipts, redemptions, and challenges
become typed state with hashes, signatures, roots, finality, and replay paths.

Crypto adoption is now buy-side, issuer, custody, market-data, compliance, and
asset-management infrastructure. Replacing SWIFT as a generic message network
is an incomplete endgame. The real gap is a base layer where financial claims,
policy roots, and permitted actions share one audit trail.

PostFiat starts from a simple commercial premise: institutions will not publish
their full portfolios, but they still need assets, reports, and workflows whose
claims can be inspected, challenged, and enforced. The chain should make the
trust boundary explicit instead of pretending off-chain facts are trustless.

## The Primitive

Everything below is built on a single object: a **signed, challengeable
record**.

A record binds the source path, content hash, attestor root, timestamp, proof
profile, and action it authorizes. The ledger then answers the question finance
actually cares about: not "is the world perfectly knowable," but *which signed
claim allowed this action, what was committed, and what happened when it was
challenged.*

The canonical pipeline:

```text
private source material
  -> signed reserve / evidence packet
  -> source root + attestor root + content hash
  -> deterministic policy check
  -> finalized ledger state
  -> mint / redeem / disclose / halt / challenge
  -> dashboard, API, or settlement action
```

Indexing sells access to the normalized record. The information network sells
controlled distribution and compliance receipts. NAV-tracked assets use the
same mechanics to gate minting, redemption, and proof freshness. None of the
three requires a general smart-contract VM in the first version; they ship as
typed records and native actions.

PostFiat is an XRPL-derived authority-settlement chain redesigned for private
financial workflows: deterministic finality, low-cost validation, fixed supply,
fee burn, Cobalt-governed trust evolution, source-bound evidence, shielded
settlement, and post-quantum authorization from genesis. Validators are natural
network participants: exchanges, custodians, issuers, gateways, index sponsors,
application operators. Their own business depends on correct settlement and
credible evidence, which is why the network can work without a validator
subsidy.

Each business need maps to a specific L1 mechanism:

| Business need | L1 mechanism |
| --- | --- |
| Low-cost institutional validators | Authority validation, current-state validation, partial-history roles, no native subsidy. |
| Publicly auditable claims | Evidence packets, content hashes, signed receipts, registry roots. |
| Governance over admissible data and rules | Cobalt-governed registries, challenge windows, fail-closed transitions. |
| Confidential financial workflows | Orchard/Halo2-style shielded settlement, disclosure receipts, metadata controls. |
| Replayable model or research outputs | Pinned profiles, typed outputs, deterministic selectors, evidence roots. |
| Durable authorization | Post-quantum account and validator authorization from genesis. |

## What's Actually Built

The current implementation gives the business case a concrete substrate: native
NAV-tracked assets and proof-of-reserves mechanics on the existing issued-asset,
trustline, offer-book, and receipt stack.

The controlled-testnet path supports:

| Claim | Evidence |
| --- | --- |
| NAV assets can be registered as native ledger objects. | `nav_asset_register` transaction path. |
| Reserve packets can be submitted and finalized. | `nav_reserve_submit` and `nav_epoch_finalize`. |
| Minting is capped by finalized reserve supply. | `nav_mint_at_nav`. |
| Redemption burns units and creates deterministic claims. | `nav_redeem_at_nav`. |
| Challenged or unsafe assets can halt. | `nav_reserve_challenge` and `nav_halt`. |
| NAV/PFT secondary liquidity uses the existing offer book. | `scripts/navcoin-current-infra-smoke`. |
| Python can build reserve packets and operation JSON. | `python/postfiat_rpc/navcoin.py` and `python/tests/test_navcoin.py`. |

The native smoke test runs the path end to end on a controlled validator
devnet: create a NAV issued asset, register it, submit and finalize a reserve
packet, mint at finalized NAV, swap NAV into PFT through the offer book, redeem
at finalized NAV, and verify validator convergence.

The proof-of-reserves path does not require public portfolio disclosure. It
requires an attested packet. A reserve operator, fund administrator, broker,
custodian, auditor, or confidential proof system can produce the packet; the
chain enforces what the packet permits.

## Trust Model and Dispute Resolution

PostFiat does not make off-chain facts trustless. It makes dependencies
explicit enough that a reviewer can see who is trusted, what they can break,
which root committed to the claim, and which actions halt while a dispute is
resolved.

| Actor | Failure mode | Control |
| --- | --- | --- |
| Validators | Censor, fork, or coordinate around ordering. | Cobalt-governed validator set, visible registry transitions, finality rules. |
| Issuer / reserve operator | Overstate assets, hide liabilities, delay redemptions, switch proof paths opportunistically. | Registered proof profile, required asset/liability fields, finalized NAV epochs, stale-proof rejection, challenged-proof halts, AP permissions, redemption receipts. |
| Attestor / fund admin / custodian | Sign a false or incomplete reserve statement. | Source roots, attestor roots, public packet hashes, challenge windows, superseding corrected packets, legal accountability. |
| Authorized participant | Mint or redeem against old, disputed, or manipulated NAV. | Mint/redeem only against finalized epochs, supply caps, permissioned AP keys, action receipts, halt states. |
| Governance actor | Capture a registry, lower a proof standard, admit a weak operator. | Cobalt old/new transition checks, published policy diffs, shadow periods for material changes, rollback to prior accepted root on failed promotion. |

The dispute path is mechanical. A submitted packet first passes deterministic
checks for schema, source root, attestor root, packet hash, epoch, arithmetic,
supply cap, issuer authority, and proof profile. Failures are rejected. A
passing packet can be finalized for the epoch. A challenger can name the
disputed field, source root, attestor root, proof profile, or packet hash. During
the challenge, the affected action enters a safe state: NAV mint/redeem halts
for that epoch until the claim expires, is corrected, or is upheld.

The invariant: **a disputed external fact cannot quietly become a financial
action.** The guarantee is bounded because proving an off-chain balance still
requires trusting a source, attestor, or proof system. PostFiat's job is to make
that trust explicit, signed, rooted, challengeable, and tied to ledger behavior.

## Product 1 - Financial Indexing

**Product.** A financial data layer over chain state, reserve claims, validator
evidence, issuer disclosures, custody events, compliance artifacts, and
market-structure records. The canonical place to ask: what happened, who signed
it, what source supported it, when did it expire, what root committed to it?

The first surface is an API and dashboard over indexed records: account and
asset activity, settlement activity, reserve and NAV evidence, validator and
operator evidence, disclosure receipts, challenge status, source freshness, and
derived metrics with provenance.

Because the L1 treats evidence as protocol material, the index is more than a
scraper. It can show whether a claim was admitted under the active registry
root, which attestor class signed it, which packet hash was used, whether it was
challenged, and whether later evidence superseded it. Validators stay cheap:
consensus participation is separated from optional archive/indexer roles.

**MVP.** Index chain state, finalized blocks, receipts, fees, balances, evidence
packet roots, source classes, attestor groups, expiry epochs, and challenge
states. Ship human-readable provenance pages for reserve, validator, disclosure,
and redemption claims.

## Product 2 - Trade Ideas and Information-Network Compliance

**Product.** An institutional intelligence network where trade ideas, research,
signals, and compliance decisions are distributed with machine-checkable lineage
instead of living in a chat room. Every actionable idea carries source lineage,
permissioning, distribution history, conflict checks, and compliance receipts.

**Workflow.** A source record enters: report, filing, market observation,
custody record, counterparty update, model output, or analyst note. It is
normalized into a typed evidence packet with source IDs, timestamps, access
class, restrictions, and content hash. A model or analyst may derive a trade
idea, but the derived object binds to the underlying evidence root.
Distribution checks permissions, wall-crossing, restricted lists, jurisdiction,
and entitlements. Delivery emits a compliance receipt: who received what class,
under which policy root, when, with which redactions.

The chain never publishes raw chat or proprietary research. It publishes
commitments, permissions, redactions, and receipts proving that the workflow was
followed. When an idea becomes a transaction or NAV-impacting event, shielded
settlement and disclosure receipts let the research network and settlement
network share one provenance grammar.

**MVP.** Packet types for source record, derived idea, distribution
authorization, and compliance receipt. Add a permissioned dashboard for approved
recipients, policy roots for restricted lists and entitlement classes, signed
delivery receipts, and replay tooling to recompute whether a recipient should
have received an idea under the policy active at delivery time.

## Product 3 - Native NAV-Tracked Assets

**Product.** NAVCOIN-style assets whose unit value tracks verified net asset
value rather than a fixed dollar promise. Each asset has a reserve operator,
authorized participants, valuation policy, reserve proof profile, proof cadence,
and redemption policy. The asset exposes current finalized NAV per unit,
circulating supply, verified asset/liability roots, reserve-packet timestamp,
proof freshness, redemption availability, and challenge status.

The commercial promise is transparent NAV tracking with enforceable mint/redeem
mechanics. If the strategy gains or loses value, NAV moves. The current NAV,
proof freshness, and redemption rules are mechanically tied to minting and
redemption. Users inspect packet roots and receipts instead of trusting issuer
prose.

**Native transaction types.** The implementation uses L1 transaction types, not
a general contract system:

| Transaction | Purpose |
| --- | --- |
| `nav_asset_register` | Registers an asset as NAV-tracked; binds issuer, reserve operator, proof profile, valuation unit, redemption account. |
| `nav_reserve_submit` | Submits a reserve/NAV packet with source root, attestor root, NAV per unit, supply, verified net assets. |
| `nav_reserve_challenge` | Marks a submitted packet challenged and halts the asset. |
| `nav_epoch_finalize` | Finalizes a NAV value for an epoch and updates the active reserve packet. |
| `nav_mint_at_nav` | Mints only against the finalized NAV epoch and finalized supply cap. |
| `nav_redeem_at_nav` | Burns units and creates a deterministic redemption claim at finalized NAV. |
| `nav_halt` | Freezes/unfreezes mint/redeem when freshness, attestation, or solvency rules require it. |

**Reserve packet.** A production packet binds custody and broker balances, cash
and collateral, open positions, liabilities and pending redemptions, valuation
marks and haircut policy, circulating supply, AP mint/redeem activity, source
timestamps, attestor signatures, proof profile, and content hashes. The chain
standardizes packet format and proof profile. The issuer picks strategies and
custodians, but the asset cannot mint or finalize NAV from an unregistered proof
path.

**Why native, not contracts.** Ethereum needs contracts because it does not know
what a NAV-tracked asset is. PostFiat makes accounting, proof cadence, challenge
window, AP minting, redemption, and halt semantics first-class ledger behavior.
A user should not have to reverse-engineer a contract suite, dashboard,
custodian note, and off-chain attestation to know whether an asset is redeemable
at the displayed NAV.

This is also why operators run infrastructure without a subsidy. A reserve
operator, custodian, index sponsor, AP, or buy-side operator benefits directly
from a credible asset and evidence rail: cheaper reconciliation, faster audit
response, cleaner client reporting, lower dispute cost. Protocol rewards attract
yield seekers; native utility attracts operators who need the system to work.

**MVP.** Harden the reserve proof profile with a real Nitro or equivalent
collector, one custody/broker source class, one attestor policy, redemption
settlement receipts, one AP flow, a dashboard for NAV/unit, supply, backing,
proof freshness, challenge status, redemption availability, and a verifier that
recomputes NAV from packet inputs and supply.

## Build Order

1. **Harden native NAV proof-of-reserves.** Replace local placeholder source
   profiles with a production collector or attestor workflow.
2. **Ship the indexing surface.** Show packet roots, receipts, balances,
   challenge states, and redemption records in one queryable surface.
3. **Add information-network compliance receipts.** Turn intelligence delivery
   into a reviewable workflow rather than an informal feed.
4. **Close redemption settlement.** Wire off-chain settlement receipts to
   pending redemption records.
5. **Keep AI outside financial authority.** Use models only for typed,
   replayable research or review outputs where deterministic rules already
   bound the action.

This keeps the first product close to existing chain/RPC/evidence work and
focuses the business case on the primitive that is already real: signed
financial claims becoming enforceable ledger behavior.

## Bottom Line

PostFiat's first business lines are evidence-native. Indexing is the data plane,
information-network compliance is the workflow plane, and NAV-tracked assets are
the asset plane. All three rest on one thesis: financial systems get more
credible when claims, permissions, roots, receipts, and challenges live in the
operating substrate instead of prose wrapped around it.
