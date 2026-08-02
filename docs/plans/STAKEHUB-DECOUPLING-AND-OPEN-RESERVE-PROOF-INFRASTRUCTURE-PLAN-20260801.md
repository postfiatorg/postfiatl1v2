# StakeHub Decoupling and Open Reserve-Proof Infrastructure Plan

**Date:** 2026-08-01
**Priority:** P0 product boundary; P0 protocol portability
**Status:** implementation in progress; Phases 0–2 complete; Phase 3 CPU and
consensus qualification complete; generic multi-route relay and controlled
qNAV lifecycle qualified; accelerated proof, live A666 governance migration,
and unaffiliated-operator reproduction remain
**Primary affected flow:** Ethereum USDC -> pfUSDC -> NAVCoin primary issue/redeem -> optional Ethereum wrapped NAVCoin
**Immediate live asset:** A666
**Historical asset affected by legacy code:** A651

## 1. Executive decision

StakeHub is an internal operator product. It is not a protocol dependency that
an external wallet user, NAVCoin issuer, reserve operator, brokerage adapter,
or independent prover can be required to possess.

The current implementation violates that boundary in several places:

1. the browser wallet names StakeHub as though it were part of the public
   reserve-verification model;
2. a legacy A651 wallet-proxy route calls StakeHub HTTP endpoints directly;
3. the unattended A666 Ethereum relay imports the StakeHub signing agent;
4. the `postfiat-node` binary contains StakeHub-specific operator workflow and
   signer-socket integration;
5. the deployed A666 proof profile and its proof-generation scripts use
   StakeHub-specific source names and hash domains; and
6. the L1's nominally generic SP1 NAV verifier decodes the fixed public-values
   ABI produced by StakeHub's six-leg aggregate program.

The correct end state is:

```text
reserve and brokerage sources
        |
        v
open PostFiat Reserve Proof Kit
  - source adapters
  - canonical manifest
  - witness construction
  - SP1 guest and prover
  - local verification
  - packet submission SDK/CLI
        |
        v
PostFiat L1 consensus
  - versioned proof profiles
  - bounded canonical public values
  - deterministic proof verification
  - freshness, policy, and replay enforcement
        |
        v
wallet and primary market
  - read finalized consensus facts
  - never call an operator portfolio product
  - never require the user to know which internal tool produced a proof
```

The existing A666 profile must not be silently reinterpreted or rewritten.
A provider-neutral successor profile must be registered and qualified, then
A666 must be migrated through an explicit governed profile/route transition.
The existing `stakehub-*` profile, packets, deployment records, and evidence
remain immutable history.

## 2. What is and is not broken

### 2.1 The core A666 swap transition is not a live StakeHub API call

The current A666 browser flow reads finalized PFTL state through:

- `navcoin_bridge_supply_status`;
- `vault_bridge_status`;
- `account_assets`; and
- ordinary chain status/RPC methods.

The wallet derives the primary-market amount from the finalized NAV, verifies
that the market policy pins the same NAV epoch and reserve packet, builds the
exact operation, and signs locally. Consensus independently enforces the
same bindings.

Therefore an ordinary user issuing or redeeming A666 against an already
finalized NAV packet does not need StakeHub access. A PFTL-resident issue or
redeem can execute without a browser-to-StakeHub request.

### 2.2 The surrounding system is nevertheless operator-coupled

The current system cannot be considered externally deployable because:

- generating a fresh A666 reserve proof depends on proof code and source
  adapters housed under StakeHub;
- producing the next finalized NAV packet is operationally available only to
  the internal reserve operator;
- the unattended Ethereum export path checks and uses StakeHub's signer;
- the return deployment uses the StakeHub Python environment and related
  scripts;
- the old A651 route explicitly forwards a wallet-proxy request to the
  StakeHub operator wallet; and
- an external NAVCoin issuer cannot start from the L1 repository and reproduce
  the complete prove, submit, issue, redeem, and export system.

The accurate product statement today is:

> A user can execute against a finalized PFTL NAV packet without StakeHub.
> Provider-neutral proof publication and unattended cross-chain components now
> exist and pass controlled qualification, but the deployed A666 proof profile
> remains on its historical internal-operator lineage until governance performs
> the explicit successor migration.

That is a transitional state, not the target architecture.

## 3. Audit scope and result

The audit searched tracked files case-insensitively for `StakeHub`,
`stakehub`, `stake_hub`, and hyphen/space variants. Generated evidence and
archived documentation were excluded from the headline count.

The initial baseline result was:

- **1,203 textual matches**;
- **109 tracked files**;
- **437 documentation matches**;
- **302 Rust/L1 matches**;
- **276 wallet-proxy matches**;
- **166 script matches**;
- **17 deployment matches**; and
- **3 browser-wallet matches**.

These are not 1,203 distinct production dependencies. Many are test fixtures,
historical reports, compatibility names, or repeated fields in large operator
workflow structures. The material dependencies are classified below.

After implementation, the shipped-code boundary scan passes with zero
provider references in browser source, browser bundle, public wallet-proxy
runtime, validator/node runtime, scripts, open tools, or the active A666 relay
deployment. The only non-documentation matches outside generated evidence are:

- the forbidden-pattern string inside
  `scripts/check-provider-neutral-wallet-boundary`; and
- one legacy replay fixture label in
  `crates/execution/src/fee_replay_execution_tests.rs`.

Neither is executable provider integration. Historical documentation and
immutable deployment/evidence records intentionally retain their original
names and are labeled as such where they could be mistaken for current
instructions.

## 4. Dependency inventory

### 4.1 Browser-wallet product language

The initial shipped wallet contained three direct StakeHub statements:

- `wallet-web/src/components/A666Market.jsx` says that the route packet
  "matches live StakeHub NAV";
- `wallet-web/src/components/NavList.jsx` says that the route and finalized
  "StakeHub reserve packet" match; and
- `wallet-web/src/components/NavDetail.jsx` describes a "finalized StakeHub
  NAV epoch."

These statements are product defects. The wallet can establish that:

- a NAV packet is finalized by PFTL;
- the active route pins the packet;
- the packet is within the governed freshness bound;
- the configured proof program and policy were accepted by consensus; and
- the route accounting invariant holds.

The wallet cannot establish that StakeHub is the relevant public institution,
nor should a user need that concept. The copy must use provider-neutral terms
such as `finalized reserve proof`, `proof profile`, `reserve packet`, and
`proof age`.

An optional human-readable provider label may appear only as non-authoritative
metadata in an advanced proof-details view. It must never change execution,
readiness, pricing, or trust classification.

**Resolution:** complete. The strings and A666-coded component/route fallback
were removed. The wallet now discovers bounded route identity, asset metadata,
precision, trust class, and wrapper bindings from finalized PFTL RPC state.
Missing or substituted registry data fails closed.

### 4.2 Legacy StakeHub transparent wallet-proxy route

The initial wallet proxy contained a complete legacy route named:

```text
stakehub_transparent_roundtrip
```

It is configured through variables including:

```text
NAVSWAP_STAKEHUB_BASE_URL
NAVSWAP_STAKEHUB_ACTION_PATH
NAVSWAP_STAKEHUB_NAVCOIN_PATH
NAVSWAP_STAKEHUB_NAVCOIN_STATUS_PATH
NAVSWAP_STAKEHUB_BALANCES_PATH
NAVSWAP_STAKEHUB_SWAP_STATUS_PATH
NAVSWAP_ENABLE_STAKEHUB_TRANSPARENT_RUNS
```

It reads StakeHub's `/api/navcoin`, balance, and swap-status responses and can
forward a round-trip action to StakeHub's operator wallet. Its own capability
description admits that it is an operator-backed A651 smoke route rather than
browser-local execution.

This route must be retired, not renamed. It represents an obsolete product
and custody boundary:

- A651 is historical;
- the route is not the A666 resident primary market;
- it is not wallet-local custody;
- it is not required by the current A666 flow; and
- preserving it in public capabilities creates continuing architectural and
  UX confusion.

The `/api/navswap/nav-proof` endpoint initially shared this configuration and
reads StakeHub's NAV response. If retained, that endpoint must be rebuilt on
PFTL RPC state. It must not proxy an operator dashboard.

**Resolution:** complete. The route, environment variables, forwarding code,
capabilities, proof proxy, and public tests were removed. Current proof status
comes from the bounded PFTL proof/profile/packet RPC.

### 4.3 A666 export and return relays

The initial unattended A666 export driver:

- imports `stakehub.agentd` during readiness;
- requires a configured StakeHub Python interpreter and repository path;
- checks that the StakeHub agent is unlocked;
- checks its contract whitelist; and
- relies on scripts that use the same agent to submit Ethereum transactions.

The initial production deployment hardcoded:

```text
/home/postfiat/repos/StakeHub/.venv/bin/python
/home/postfiat/repos/StakeHub
```

The initial return relay also selected the StakeHub Python environment, and its burn
inspection helper contains an optional StakeHub-agent transaction path.

This is not a user-custody dependency: the user still signs the PFTL source
operation and, on return, the MetaMask burn. It is nevertheless an availability
and external-operator portability dependency. A third-party route operator
cannot reproduce the relay from the PostFiat release alone.

The replacement must be a narrow, open-source signing/relaying service with a
stable provider-neutral interface. StakeHub may temporarily implement that
interface behind a compatibility adapter, but no shipped wallet or relay may
import StakeHub directly.

**Resolution:** complete in source and deployment configuration. Export and
return now use route-generic durable supervisors, generic drivers, and the open
constrained signer. Up to 64 distinct route configs can coexist; job identity
binds the case-sensitive route ID. The checked A666 deployment configuration
loads against the generic schemas and exact driver hashes. Live service
cutover remains governed deployment work, not a code dependency.

### 4.4 StakeHub-specific workflows inside `postfiat-node`

The removed `postfiat-node nav-roundtrip-live-demo` operator workflow accepted fields
such as:

```text
--stakehub-home
--stakehub-wallet
STAKEHUB_HOME
~/.stakehub/agent.sock
```

It opens and closes StakeHub signing sessions, sends EVM contract operations
through the agent, and emits StakeHub-specific report fields and timings.

These facilities were not consensus. They were an embedded campaign harness.
Their presence in the node binary confused the protocol boundary and forced
an L1 release to understand one internal operator product.

The node must retain generic primitives for:

- transaction construction and fee quotation;
- signed transaction submission;
- bridge evidence and receipt construction;
- NAV profile and packet queries;
- checkpoint and finality proof construction;
- deterministic state verification; and
- route/accounting status.

Operator-specific session management, EVM wallet control, portfolio fetching,
and campaign orchestration must move to an open SDK/CLI outside the validator
runtime.

**Resolution:** complete. The embedded provider-specific campaign modules,
CLI dispatch, tests, and scripts were removed. The node retains the generic
consensus, query, transaction, checkpoint, and proof primitives. Release
binary scanning is an acceptance gate before publication.

### 4.5 The existing L1 proof-profile substrate

The L1 already contains a useful provider-neutral base. `NavProofProfile` is
content-addressed and binds:

- verifier kind;
- source class;
- freshness and challenge windows;
- attestation requirements and tolerance;
- valuation-policy hash;
- SP1 program vkey and proof encoding; and
- proof/public-value byte bounds.

`NavReserveSubmitOperation` binds the submitted packet to:

- issuer and authorized submitter;
- NAV asset ID;
- epoch;
- NAV per unit;
- circulating supply;
- verified net assets;
- proof profile;
- source and attestor roots; and
- SP1 proof/public values.

Consensus checks collateralization, submitter authority, profile identity,
proof size, SP1 verification, verified-net-assets equality, policy-hash
equality, packet uniqueness, freshness, and later route/policy pinning.

These mechanisms must be extended rather than replaced with a parallel
registry.

### 4.6 The hidden consensus coupling: fixed aggregate public values

Before this remediation, `sp1-groth16` accepted the governed program vkey but
then decoded
the proof output as StakeHub's `AggregatePublicValuesV2`. The decoder assumes
fixed ABI offsets and derives:

```text
verified_net_assets = spot_total + cash_total - liability
```

It then checks only the decoded valuation-policy hash and verified net assets
against the profile and packet.

Those legacy public values do not explicitly bind a standard PostFiat proof
context containing all of:

- PFTL chain/genesis domain;
- NAV asset ID;
- proof-profile ID;
- source-manifest hash;
- valuation unit and precision;
- observation epoch or bounded observation interval; and
- provider-neutral trust-breakdown commitments.

Some of this can be bound indirectly through a private policy-hash preimage,
but indirect convention is not an adequate public protocol standard.

**Resolution:** complete through a versioned successor, without changing
legacy semantics. `NavReservePublicValuesV1` is a bounded canonical 584-byte
ABI that binds the full chain, asset, profile, manifest, policy, observation,
totals, and trust-class context. The legacy decoder remains only for immutable
historical profiles.

### 4.7 Proof generation and per-source adapters

The initial private proof implementation included substantial
useful work:

- SP1 aggregate guest and prover;
- shared canonical encoding;
- EVM/Aave and EVM spot verification;
- Hyperliquid receipt and attested modes;
- Solana and NEAR receipt/attested modes;
- XMR reserve proof integration;
- valuation reconciliation; and
- proof fixtures and contract adapters.

It is nevertheless specialized:

- hash domains use `stakehub-*` names;
- `LegKind` is a fixed enum of six known sources;
- the maximum is fixed at six legs;
- expected reserve owners/signers can be compiled into the guest;
- witness-building commands assume StakeHub's directory and operating model;
  and
- the L1 repository contains proof fixtures and verification code but not the
  complete source-adapter/prover product needed by another issuer.

**Resolution:** complete for the reference implementation. The open
`tools/nav-reserve-proof` kit contains manifest-driven shared types, guest,
host, CLI, packet construction/submission, controlled fixtures, Ed25519
attestation/protocol-receipt support, and the EVM ERC-20 BFT-checkpoint/MPT
adapter. It uses versioned PostFiat domains and has no internal-product import,
path, API, or credential requirement. Additional source adapters can be added
under the same bounded manifest and trust-class rules.

### 4.8 Deployed A666 identity

The deployed A666 profile uses:

```text
verifier_kind = sp1-groth16
source_class  = stakehub-six-leg-reserves-v2
```

Several A666 scripts also use domains including:

```text
postfiat.a666.stakehub_source_root.v1
postfiat.a666.stakehub_attestor_root.v1
```

These values are part of already-signed/deployed history. They must not be
globally search-and-replaced. Changing a hash-domain string changes consensus
identities and packet hashes.

The correct remediation is an immutable successor profile and new versioned
domains.

### 4.9 Documentation, tests, and historical evidence

StakeHub references fall into three documentation classes:

1. **Normative/current:** architecture, wallet, deployment, and runbook text
   that describes StakeHub as a required public component. This must be
   corrected.
2. **Historical:** dated evidence, postmortems, completed campaign reports,
   and old deployment manifests. This must remain unchanged, with a banner
   where needed explaining that it records a retired architecture.
3. **Implementation tests:** tests for the retired A651 adapter or embedded
   node workflow. These must be deleted or rewritten with the code they test.

Historical accuracy is more important than achieving a repository-wide zero
match. The zero-match gate applies to shipped runtime paths, not immutable
history.

## 5. Required ownership boundary

### 5.1 What belongs in L1 consensus

L1 consensus must own deterministic verification of:

- proof-profile registration and immutable identity;
- the accepted public-values schema;
- SP1 vkey and proof encoding;
- proof and public-value size limits;
- asset/profile/policy/source-manifest bindings;
- NAV unit and arithmetic;
- packet epoch, freshness, and replay protection;
- verified assets, liabilities, and net assets;
- trust-class commitments;
- collateralization;
- finalized packet history; and
- primary-market policy pinning.

Validators must not call exchanges, custodians, brokerages, Ethereum RPCs,
or operator services while executing consensus transitions.

### 5.2 What belongs in the open reserve-proof kit

The open kit must own:

- source observation adapters;
- ownership challenges;
- signed statement and receipt parsing;
- witness normalization;
- canonical source ordering;
- price and haircut inputs governed by the valuation policy;
- SP1 guest execution and proof production;
- native reconciliation;
- local proof verification;
- packet construction; and
- submission through ordinary PFTL RPC.

The prover is not trusted. Consensus trusts only a valid proof under the
registered vkey and the exact public bindings.

### 5.3 What belongs in the wallet

The wallet must:

- discover NAVCoins and routes from registries/RPC;
- read the active proof profile and finalized reserve packet;
- display freshness and trust classification;
- verify route/NAV/policy agreement;
- build and sign the user's exact operations locally; and
- track finality and recovery.

The wallet must not:

- fetch a fund's brokerage positions;
- construct the issuer's NAV witness;
- call StakeHub;
- require a StakeHub URL or token;
- infer proof quality from a provider brand; or
- claim that an attested source is cryptographic merely because an SP1 proof
  aggregated it.

### 5.4 What belongs in operator infrastructure

An issuer or route operator may run:

- the open reserve-proof kit;
- private brokerage credentials and source adapters;
- a local or remote SP1 prover;
- a generic constrained signer;
- packet publication automation; and
- bridge relayers.

Those services may be privately operated. Their implementations and protocol
interfaces must be publicly reproducible, and their outputs must be verified
by L1 rather than trusted by the wallet.

## 6. Provider-neutral proof standard

### 6.1 Versioned public values

Add a successor schema, provisionally:

```text
postfiat.nav_reserve_public_values.v1
```

Its fixed, bounded public values must bind at least:

```text
schema_version
pftl_genesis_hash
nav_asset_id
proof_profile_id
valuation_policy_hash
source_manifest_hash
valuation_unit_id
valuation_scale
observation_epoch
observation_not_before
observation_not_after
source_observation_root
gross_assets
total_liabilities
verified_net_assets
cryptographically_verified_value
attested_value
source_count
source_disclosure_root
```

The exact encoding must be canonical and platform-independent. Integers must
have explicit widths and units. Floating-point values, maps with unstable
iteration order, unbounded vectors, wall-clock calls inside consensus, and
implicit string normalization are forbidden.

The public-values schema must reject:

- a proof for another PFTL genesis;
- a proof for another NAV asset;
- a proof under another profile or valuation policy;
- a source manifest substituted after registration;
- a stale or future observation interval;
- inconsistent gross, liability, and net totals;
- unbounded or duplicate sources;
- non-canonical source ordering; and
- arithmetic overflow or underflow.

### 6.2 Source manifest

Each profile must bind a canonical, content-addressed source manifest. A
source entry should include:

```text
source_id
adapter_kind
source_domain
asset_or_position_id
reserve_owner_commitment
quantity_verifier_commitment
valuation_verifier_commitment
quantity_evidence_class
valuation_evidence_class
freshness_policy
haircut_policy
liability_treatment
adapter_schema_version
```

In public-values v1, `total_liabilities` is supported as a bounded amount
attributable to an asset source. A standalone `Liability` source is rejected:
the v1 trust-value buckets are unsigned and cannot faithfully represent a
negative source. Supporting standalone liabilities requires a successor ABI
with explicit signed or separate per-class asset/liability totals.

The manifest must be bounded and deterministically ordered. Public metadata
may include a documentation URI, but consensus must use the committed hash,
not the URI contents.

Adding a source, changing an account, changing a haircut, or changing a trust
class creates a new manifest and proof profile. It must not silently alter an
existing profile.

### 6.3 Trust classifications

The proof standard must distinguish at least:

- `CRYPTOGRAPHIC`: quantity or valuation is derived from evidence verified
  cryptographically against an accepted root or signed protocol receipt;
- `ATTESTED`: a bounded, identified attestor signs the observation, but the
  underlying venue state is not independently proven; and
- `CONTROLLED`: suitable only for controlled demonstrations and forbidden
  for public live-value claims unless a separate policy explicitly permits
  it.

Quantity and valuation need separate classifications. For example, an EVM
balance can be cryptographically proven while its USD mark remains attested.

An SP1 aggregate proof proves that its program processed its inputs correctly.
It does not make an unverifiable brokerage API response true. The wallet and
RPC must preserve that distinction.

## 7. Open Reserve Proof Kit

The recommended home is inside the public L1 repository so the consensus ABI,
reference guest, fixtures, and submission tooling evolve together. A separate
public repository is acceptable only if releases are pinned and tested by the
L1 workspace.

Proposed structure:

```text
tools/nav-reserve-proof/
  crates/
    reserve-proof-types/
    reserve-proof-manifest/
    reserve-proof-guest/
    reserve-proof-prover/
    reserve-proof-cli/
    adapter-evm/
    adapter-hyperliquid/
    adapter-solana/
    adapter-near/
    adapter-xmr/
  fixtures/
  manifests/
  docs/
```

Required CLI workflow:

```text
postfiat-reserve-proof manifest validate
postfiat-reserve-proof observe
postfiat-reserve-proof witness build
postfiat-reserve-proof execute
postfiat-reserve-proof prove
postfiat-reserve-proof verify
postfiat-reserve-proof packet build
postfiat-reserve-proof packet submit
```

The kit must support local CPU execution for correctness and an authenticated
remote prover interface for acceleration. A remote prover receives a bounded
witness and returns a proof; it receives no signing authority and is not
trusted for correctness.

The first open state-proof adapter is
`evm-erc20-bft-checkpoint-mpt-v1`. It verifies ERC-20 account/storage MPT
proofs beneath a quorum-signed, manifest-pinned EVM state root and exposes its
actual trust boundary as BFT-checkpoint quantity evidence. This is not silently
described as trustless Ethereum finality. Price evidence remains an independent
manifest commitment and trust class.

Builds must be reproducible enough that an external operator can derive the
same guest ELF hash and program vkey from the tagged source/toolchain.

## 8. Generic signer and relayer boundary

Create a standalone open-source signer service, provisionally
`postfiat-signer`, or an equivalent library/service pair.

Its request must bind:

```text
chain_id
transaction_kind
target_contract
calldata
native_value
maximum_fee
route_id
route_config_digest
human-readable label
idempotency_key
```

Its policy must support:

- permitted chain IDs;
- exact contract allowlists;
- permitted function selectors;
- per-call and rolling value limits;
- maximum gas/fee;
- route and deployment digest pins;
- locked/unlocked state;
- hardware or local-keystore backends;
- durable idempotency; and
- auditable receipts without exposing private keys.

During migration, a StakeHub adapter may implement this interface. The A666
relay must depend only on the generic interface. Once the standalone signer
passes the same policy tests, the StakeHub adapter is removed.

## 9. Phased remediation plan

### Phase 0 — immediate wallet and API containment

This phase is deliberately narrow and must not rotate the live A666 proof
profile or change consensus before the time-sensitive demonstration.

- [x] Replace the three browser-wallet StakeHub strings with
  provider-neutral finalized-proof language.
- [x] Rebuild and inspect the production wallet bundle.
- [x] Remove `stakehub_transparent_roundtrip` from public capability, quote,
  run, status, and persistence APIs.
- [x] Remove or rewrite its wallet-proxy tests.
- [x] Rebuild `/api/navswap/nav-proof` on PFTL RPC, or remove the endpoint if
  no current wallet consumer needs it.
- [x] Add a shipped-code CI check that rejects StakeHub references in
  `wallet-web`, public wallet-proxy routes, and production browser bundles.
- [x] Update current wallet/product documentation.
- [x] Leave historical evidence and the live A666 profile unchanged.

**Phase 0 exit condition:** a user can operate the current wallet without
seeing, configuring, or calling StakeHub. Existing A666 NAV publication may
remain temporarily operated by the internal backend, documented as a known
operator-availability dependency.

### Phase 1 — generic signer compatibility boundary

- [x] Specify the signer request, response, error, readiness, and idempotency
  schemas.
- [x] Implement the generic client in the A666 export and return relays.
- [x] Implement a temporary StakeHub compatibility adapter. This became
  unnecessary because the relays cut directly to the standalone signer; no
  compatibility adapter entered production.
- [x] Replace absolute StakeHub repository and Python paths in deployment
  configuration.
- [x] Add wrong-chain, wrong-contract, wrong-selector, excess-value,
  duplicate-request, locked-agent, and restart tests.
- [x] Implement and qualify the standalone signer backend.
- [x] Remove the compatibility adapter from production. No production
  compatibility adapter exists after the direct cutover.

### Phase 2 — L1 provider-neutral proof ABI

- [x] Define and review `NavReservePublicValuesV1`.
- [x] Extend the existing proof-profile model with public-values schema and
  source-manifest commitments.
- [x] Add a new verifier kind/version; do not mutate existing
  `sp1-groth16` decoding semantics.
- [x] Implement canonical decoding and all context bindings.
- [x] Add a dedicated NAV proof/profile/packet RPC suitable for wallets and
  explorers.
- [x] Commit every new field to the state root and snapshots.
- [x] Add legacy replay and migration coverage.
- [x] Fuzz the public-values decoder and proof operation parser.
- [x] Run deterministic six-validator replay tests.

### Phase 3 — extract and generalize the proof kit

- [x] Move or port the existing StakeHub ZK shared types, guest, prover, and
  useful source adapters into the open kit.
- [x] Replace StakeHub hash domains with new versioned PostFiat reserve-proof
  domains.
- [x] Replace the fixed six-leg enum with bounded manifest-driven source
  entries.
- [x] Replace compile-time expected owners/signers with manifest/profile
  commitments.
- [x] Preserve separate quantity and valuation trust classes.
- [x] Add canonical source ordering and duplicate-source rejection.
- [x] Publish toolchain pins, reproducible build instructions, fixtures, and
  license.
- [ ] Demonstrate CPU execute/verify and accelerated Groth16 proving. CPU
  Groth16 proving, host verification, exact consensus verification, packet
  construction, and tamper rejection pass. This host has no CUDA device and
  no authenticated network-prover credential, so the accelerated half remains
  an explicit open gate.

### Phase 4 — A666 shadow qualification and migration

- [x] Express A666's current reserve sources as a provider-neutral manifest.
- [x] Generate old and successor proofs from identical historical observations.
- [x] Reconcile verified assets, liabilities, net assets, and trust classes.
- [x] Qualify the successor ABI through transparent/private issue, redeem,
  export, return, replay, restart, and conservation tests using the distinct
  zero-value qNAV asset.
- [x] Generalize the browser registry and durable export/return relays so the
  A666 successor uses registered route data rather than A666-coded workers.
- [ ] Run shadow proofs across multiple fresh NAV epochs.
- [ ] Register the immutable successor A666 proof profile.
- [ ] Submit and finalize a successor reserve packet.
- [ ] Advance/rebind the A666 primary route through governance.
- [ ] Verify transparent and private issue/redeem against the new packet.
- [ ] Verify Ethereum export and return using the generic signer.
- [ ] Preserve all old profiles and packets as historical state.

No live route changes until the successor path passes shadow reconciliation.
Failure before activation leaves the existing route untouched. Failure after
activation must pause affected operations and use a governed recovery action;
operators must not edit profile semantics or validator state.

### Phase 5 — external-operator qualification

An operator with no StakeHub checkout, StakeHub API, internal filesystem, or
PostFiat employee intervention must be able to:

- [ ] clone tagged public source;
- [ ] build the node, wallet, reserve-proof kit, and signer;
- [ ] create a bounded source manifest;
- [ ] register a NAVCoin and successor proof profile;
- [ ] collect source evidence using documented adapters;
- [ ] build, execute, prove, locally verify, and submit a reserve packet;
- [ ] finalize the NAV packet on a six-validator environment;
- [ ] execute transparent primary issuance and redemption;
- [ ] execute private-middle issuance and redemption;
- [ ] export and return the wrapped Ethereum asset; and
- [ ] independently reproduce the public values, profile ID, packet hash,
  and trust classification.

At least one qualification asset must use a source manifest different from
A666. Otherwise the project has demonstrated an A666-specific extraction,
not a general NAVCoin platform.

The controlled qNAV run now covers the protocol lifecycle and a manifest
different from A666. It is intentionally not checked off above: `G6` requires
reproduction by an unaffiliated operator from a tagged public checkout, not a
second asset operated by the implementation team.

## 10. Acceptance gates

| Gate | Pass condition |
|---|---|
| `G0` — wallet boundary | Production wallet and public proxy expose no StakeHub route, copy, URL, token, or configuration requirement. |
| `G1` — L1 standard | A versioned, bounded, canonical public-values schema binds chain, asset, profile, manifest, policy, time, totals, and trust breakdown. |
| `G2` — adversarial verification | Wrong asset/genesis/profile/vkey/policy/manifest, stale time, malformed encoding, overflow, duplicate source, tampered proof, and replay all fail deterministically. |
| `G3` — reproducible kit | A fresh public checkout builds the same guest/vkey, executes fixtures, proves, verifies, and constructs a valid packet without StakeHub. Canonical CPU proving is sufficient for this reproducibility gate; CUDA or authenticated network proving is a separate operational-acceleration sub-gate. |
| `G4` — generic signer | Export/return operate through the constrained signer interface with no StakeHub import or absolute StakeHub runtime path. |
| `G5` — A666 migration | Old and successor proof results reconcile; the governed successor profile completes transparent/private issue, redeem, export, and return. |
| `G6` — independent asset | A second asset/operator with a distinct manifest completes the same lifecycle without internal tooling. |
| `G7` — accurate trust UX | Wallet and RPC distinguish cryptographic, attested, and controlled quantities/valuations without provider-brand inference. |

## 11. Required adversarial tests

The protocol and kit test plan must include:

- wrong PFTL genesis or chain-domain proof;
- valid proof for the wrong NAV asset;
- valid proof under the wrong profile or vkey;
- valuation-policy and source-manifest substitution;
- stale, future, inverted, or overlong observation interval;
- duplicate source IDs and non-canonical order;
- source count and proof/public-value size limits;
- malformed ABI lengths and offsets;
- gross-assets, liability, and net-assets inconsistency;
- mixed valuation units or precision confusion;
- every checked arithmetic overflow/underflow boundary;
- replayed packet, epoch, observation root, and signer idempotency key;
- attested input mislabeled as cryptographic;
- reserve account/signature substitution;
- signer wrong-chain, wrong-contract, wrong-selector, and excess-fee attempts;
- validator restart and snapshot replay with the new profile fields;
- state-root equality across all validators; and
- legacy A666 profile and packet replay after the upgrade.

No parser handling public proof material may panic on malformed input. All
collections and byte payloads must be bounded before expensive verification.

## 12. Documentation migration policy

Current normative documents must be amended to use these terms:

```text
finalized PFTL reserve proof
registered proof profile
source manifest
valuation policy
reserve-proof operator
proof program/vkey
cryptographic or attested source evidence
```

They must not say that a public user needs StakeHub or that StakeHub itself is
the trust anchor.

Historical documents, evidence directories, deployed operations, transaction
hash preimages, and old profile names must remain unchanged. If a historical
document is likely to be mistaken for a current runbook, add a dated banner:

> Historical architecture: this record used the internal StakeHub operator
> path. It is retained for evidence integrity and is not the current public
> wallet or reserve-proof architecture.

## 13. Explicit non-goals

This plan does not:

- put brokerage credentials or API calls into validators;
- require ordinary swap users to run a prover;
- require every reserve source to be cryptographically trustless;
- pretend that ZK proves the truth of an unverifiable external statement;
- replace the existing NAVCoin primary-market accounting model;
- mutate the deployed A666 proof profile;
- delete historical evidence to obtain a zero-text-match result; or
- require the full migration before the immediate wallet wording and legacy
  route are corrected.

## 14. Definition of done

Current implementation status:

| Item | Status on 2026-08-01 |
|---|---|
| Wallet/public proxy boundary | PASS in source, tests, and rebuilt browser bundle. |
| Finalized-state/registered-route execution | PASS; the wallet has no static production NAVCoin registry or A666 identity fallback. |
| Provider-specific node workflow removal | PASS in source and all-target compilation; release binary scan is rerun for each release artifact. |
| Public reserve-proof implementation | PASS for `G3`: a clean checkout reproduces the guest/vkey and completes CPU execute/prove/verify, packet construction, consensus verification, and tamper rejection. The optional operational-acceleration sub-gate remains OPEN because this host has neither CUDA nor an authenticated network-prover credential. |
| Full public-values bindings | PASS. |
| A666 governed successor live | OPEN; prohibited before fresh shadow epochs, authority approval, and a separate validator rollout. |
| Generic constrained-signer export/return | PASS in source, policy tests, multi-route durable-job tests, and checked A666 deployment config; live successor cutover remains part of the A666 rollout. |
| Unaffiliated distinct-asset operator | OPEN; controlled qNAV proves the lifecycle but does not substitute for unaffiliated reproduction. |
| Shipped-code provider scan | PASS; remaining names are the scan rule, one legacy replay fixture, this migration plan, and labeled historical records. |

StakeHub decoupling is complete only when:

1. the production wallet and public proxy contain no StakeHub dependency;
2. wallet execution derives exclusively from finalized PFTL state and
   registered route data;
3. the node binary contains no StakeHub signer socket, wallet, session, or
   operator workflow;
4. the complete reference reserve-proof implementation is publicly buildable;
5. proof public values explicitly bind asset, chain, profile, manifest,
   policy, observation interval, totals, and trust classes;
6. the A666 successor profile is governed, qualified, and live without
   rewriting history;
7. export and return use the generic constrained signer;
8. an independent operator can onboard and operate a distinct NAVCoin without
   StakeHub; and
9. a repository scan finds `StakeHub` only in explicitly historical evidence,
   migration fixtures, and compatibility records that are not shipped or
   executable.

The governing architectural rule is:

> No new NAVCoin, reserve source, brokerage integration, NAV refresh, wallet
> quote, primary-market operation, or bridge operation may require the
> StakeHub package, filesystem, API, agent socket, credentials, or naming.

## 15. Qualification ledger

The following gates were rerun against the implementation on 2026-08-01 and
2026-08-02. They are local and remote qualification evidence, not a claim that
the open live-rollout or unaffiliated-operator gates have occurred.

| Surface | Command or artifact | Result |
|---|---|---|
| Provider-neutral boundary | `scripts/check-provider-neutral-wallet-boundary` | PASS; no provider reference or hard-coded A666 identity in shipped wallet/proxy runtime. |
| Browser wallet | `node --test --test-reporter=dot src/lib/*.test.js` | PASS; 228/228. |
| Wallet proxy | `node --test test_*.js` | PASS; 31/31 after removal of the unused A666-specific workflow module. |
| Browser production bundle | `npm run build` | PASS; rebuilt bundle also passes the provider-neutral boundary scan. |
| Proof kit | `cargo test --locked` | PASS; 4 CLI tests and 11 proof-type/adapter tests. |
| SP1 host integration | `cargo check --locked -p postfiat-reserve-proof --features sp1` | PASS. |
| Guest identity | committed ELF and `program-identity.json` | PASS; canonical SP1 6.3.1 Docker builds from two distinct checkout paths match at SHA-256 `0f8476431677bfe0a8f9f19db7439abce1a879ba5736cfa3225ae7de4e5b0e52`, vkey `0x000c7271e0711abce0c61d293222fd4a144599a779db8cadadc4df35e31a4100`. |
| Canonical real proof | CPU Groth16 prove and independent host verify | PASS; the controlled chain-bound fixture and consensus calldata were regenerated for the canonical identity. |
| Clean-checkout reproduction | detached checkout of `931a053af4c5debf14789498dfbe8146912d6310` in `/home/postfiat/tmp/reserve-proof-final-931a053` | PASS; the pinned Docker build reproduced the committed ELF and vkey exactly, the fresh release CLI passed all 15 proof-kit tests, manifest/profile/observe/witness/execute passed, and a second CPU Groth16 proof independently verified. Public values were deterministic at SHA-256 `95bc0bc04ddb66dac961911754111bf0fc4f56f6d4641f75991d6003d3a16d64`; proof bytes are not required to match because Groth16 proving is randomized. |
| Execution | `cargo test -p postfiat-execution` | PASS; 176/176, including real-proof, tamper, lifecycle, and controlled-source policy checks. |
| Node regression | `cargo test -p postfiat-node --lib` | PASS; 251 passed, 0 failed, 2 intentionally ignored local-Anvil tests. |
| Six-validator proof finality | `provider_neutral_qnav_proof_finalizes_and_survives_six_validator_restart` | PASS in 213.42 seconds under the canonical profile identity. |
| Node compilation | `cargo check -p postfiat-node --all-targets` | PASS. |
| Constrained signer | `python3 -m pytest -q python/tests/test_constrained_signer.py` | PASS; 10/10, including bounded durable-state rejection. |
| Release artifacts | release `postfiat-node` and `fastswap_wallet_service` | PASS; both built and binary string scans contain no StakeHub route, path, socket, configuration, or provider name. |
| Formatting and patch integrity | `cargo fmt --all -- --check`; `git diff --check` | PASS. |
| Remote product-security CI | [GitHub Actions run 30723966367](https://github.com/postfiatorg/postfiatl1v2/actions/runs/30723966367) at `931a053af4c5debf14789498dfbe8146912d6310` | PASS on 2026-08-02; all seven jobs completed successfully, including the canonical Docker guest-identity rebuild in `open-reserve-proof-kit`. `official-mainnet-fork` passed only as the explicit `UNCONFIGURED` gate because `ETHEREUM_MAINNET_RPC_URL` was absent; it is not evidence of a live fork run. |

The audit also corrected three fail-closed defects before recording these
results:

1. a profile forbidding controlled sources now rejects controlled quantity or
   valuation sources even when their current net value is zero;
2. the wallet detects duplicate asset and wrapper identities across the whole
   governed route registry rather than only adjacent rows; and
3. browser signing rejects atom values above JavaScript's exact integer range
   instead of silently rounding a valid `u64` operation.

The external gates remain exactly the unchecked items above: accelerated
proving on CUDA or an authenticated network prover, multiple fresh A666 shadow
epochs, governed A666 successor activation and live generic-relay exercise,
and reproduction by a genuinely unaffiliated operator. The Monday A666 demo
checkout remains frozen and is not a prerequisite or target of these changes.
