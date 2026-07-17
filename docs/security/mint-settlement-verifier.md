# Settlement-backed mint verifier

## Status and trust model

`ThresholdMintSettlementVerifier` is the production-candidate verifier for the
EVM `MintController` settlement-release boundary. It is an explicitly federated
`n-f` BFT certificate, not an Ethereum light client and not a trustless PFTL
finality proof. For a committee of `n` bridge signers, the constructor requires
exactly `n - floor((n - 1) / 3)` distinct signatures. A 4-member committee
therefore requires exactly 3 signatures; a 6-member committee requires exactly
5.

The bridge signer keys are secp256k1 ECDSA keys because the verifier executes on
Ethereum. This is a classical compatibility boundary and does not inherit the
post-quantum security of PFTL's ML-DSA validator signatures. Governance must map
each bridge key to a distinct validator/operator and protect it with production
signer custody before real value is enabled.

Any relayer may submit a certificate. Relayers hold no release authority: the
contract accepts a settlement only after recovering a sorted, distinct exact
quorum from the immutable committee.

## Signed domain

One verifier deployment is immutable for one PFTL authority epoch and one
`MintController`/asset-token pair. Its certificate digest binds:

- EVM chain ID and verifier contract address;
- PFTL chain-ID hash, genesis commitment, protocol version, and authority epoch;
- the computed committee root and exact BFT threshold;
- mint-controller and asset-token addresses;
- accepted envelope/pending ID and mint-escrow ID;
- recipient and exact amount atoms;
- exact settled-proceeds and locked-liquidity values;
- PFTL finalized height, 48-byte state root, 48-byte accepted-receipt hash, and
  48-byte route-config digest; and
- the exact `keccak256("accepted")` receipt code.

The resulting settlement ID is the only `proof_hash` accepted by
`MintController`. A certificate for another amount, escrow, recipient,
controller, token, verifier deployment, EVM chain, PFTL domain, finality root,
receipt, route, or receipt code cannot authorize release.

The contract enforces low-`s` ECDSA recovery, sorted unique signers, the exact
quorum count, bounded committee size, nonzero economic fields, exact 48-byte
PFTL commitments, and one-time settlement registration. The mint controller
separately consumes each settlement hash once and applies its post-mint backing
check before transferring escrowed assets.

## Deployment and rotation

Initial verifier installation requires the deployer to supply its exact runtime
code hash. `MintController` records that hash and rechecks `EXTCODEHASH` on every
release.

Verifier rotation is never immediate:

1. the controller owner schedules a new verifier and exact runtime code hash;
2. scheduling fails while any mint escrow is unresolved;
3. a fixed two-day on-chain delay must elapse;
4. activation again requires zero unresolved escrows and the same runtime code
   hash; and
5. only then does the new verifier become authoritative.

A mint request created during the delay also blocks activation until it is
released. This drain fence prevents a verifier change from reinterpreting or
stranding an existing escrow. The owner may cancel a pending rotation, but may
not replace the active verifier through the one-time initialization method.

Deployment governance must commit the verifier address, runtime code hash,
controller, token, committee root, authority epoch, and activation transaction.
Mock verifiers are test-only and are prohibited in a production manifest.

## Required release evidence

Source completion does not by itself authorize real value. Promotion still
requires:

- isolated signer tooling that derives the exact Solidity digest and never
  exports bridge private keys;
- a pinned-fork deployment test and a controlled test-environment release;
- one accepted PFTL settlement certificate releasing exactly one matching mint
  escrow on EVM;
- aggregate PFTL/Ethereum supply and backing conservation through mint, return,
  failed settlement, replay, and verifier rotation; and
- candidate code hashes and governance records tied to the immutable release
  commit.

The public product must continue to identify this as a BFT committee trust
model. It must not call it trustless finality unless a separately reviewed
on-chain PFTL finality verifier replaces the committee certificate.
