# pfUSDC on Arc: audit scope request for Zellic

**Date:** 2026-09-02
**Status:** Request for quotation and technical review. Zellic has not reviewed,
audited, endorsed, or co-sponsored anything as of this document.

## What we are asking for

1. A scoped security audit of the pfUSDC ↔ Arc bridge listed below, bound to a
   named commit, with a remediation re-review.
2. Independent review of the technical claims in the
   [public post](pfusdc-on-arc-20260902.md) using the
   [claim-classification packet](zellic-review-packet-20260902.md), so that
   Zellic can decide whether to co-sponsor the Circle Developer Grants
   application. Every claim is pre-classified; nothing in the post is worded as
   reviewed until Zellic says so.

## Scope

Source: [github.com/postfiatorg/postfiatl1v2](https://github.com/postfiatorg/postfiatl1v2),
branch `integrate/arc-tier4-current-v2-20260901`. The audit commit will be the
branch head at engagement start; the current head is recorded in the PR.

| Component | Path | Lines | Role |
| --- | --- | ---: | --- |
| Vault | `crates/ethereum-contracts/src/ERC20BridgeVaultV2.sol` | 271 | Holds USDC on Arc; deposit log is the ingress proof input; releases only against a verified PFTL finality proof |
| Ingress anchor | `crates/ethereum-contracts/src/PfUsdcIngressAnchorV1.sol` | 144 | Direct-mode deposit record and replay key; defense in depth behind the proof |
| Finality verifier | `crates/ethereum-contracts/src/PFTLFinalityVerifierV1.sol` | 555 | Verifies the Groth16 PFTL finality proof, checkpoint advancement, nullifier and withdrawal replay guards |
| One-shot factory | `crates/ethereum-contracts/src/ArcPfUsdcDeploymentFactory.sol` | 78 | Resolves the anchor/vault circular pin with no setter or upgrade path |
| Arc ingress guest | `programs/pfusdc-arc-ingress/src/lib.rs` | 1,242 | SP1 program: Arc commit-certificate quorum, header binding, receipt/MPT inclusion, deposit-log equality, exact-block EIP-1186 validator-registry proof and set transition |
| Egress guest | `programs/pfusdc-egress/` + `crates/pfusdc_proofs/` | — | SP1 program: PFTL finality with ML-DSA (FIPS-204) verified in-circuit, burn inclusion, checkpoint ancestry |
| PFTL admission | `crates/execution/src/nav_sp1_verifier.rs`, `nav_vault_asset_execution.rs` (Arc route paths) | 1,061 + | Bounded Groth16 admission, route/profile binding, mint cap and conservation accounting |
| Capture/prover host | `tools/pfusdc-tier4-prover/` | — | Witness capture from Arc RPC/archive node, negative suite, proving wrapper |
| Deployment | `crates/ethereum-contracts/script/DeployArcPfUsdc*.s.sol`, `crates/arc-sp1-contracts/` | — | Immutable bindings, SP1 v6.1.0 verifier route, readbacks |

Out of scope: PFTL consensus itself, NAVCoin logic beyond the pfUSDC settlement
coupling, the deprecated Arbitrum/Nitro transport path.

## Threat model to review against

Dishonest Arc RPC or proof provider; forged or sub-quorum Arc certificate;
validator-set rotation and registry proxy upgrade; malformed receipt/MPT/ABI
data; malicious relayer; wrong ELF/vkey/verifier route; PFTL Byzantine minority
and committee transition; rejected transaction inside a finalized block; replay
across deposits, routes, epochs, chains, proofs, and withdrawals;
malicious/reentrant/non-standard token; pause and privileged-key misuse;
checkpoint races and stale proofs; resource exhaustion in host, guest, and RPC
inputs; snapshot/restart/replay faults; prover unavailability.

## Trust assumptions we assert

- Ingress: Arc's permissioned validator quorum (≥ 2/3 voting power), verified
  in-circuit from the registry state proven at the exact deposit block.
- Egress: PFTL consensus soundness plus SP1/Groth16 soundness, plus (on the
  current testnet deployment only) non-misuse of the SP1 gateway owner key;
  see the open finding below.
- No committee, attestation layer, or operator checkpoint write exists. Two
  administrative keys exist on the testnet deployment: vault `setPaused`
  (liveness only) and the SP1 gateway `owner()`.

## Open finding we want reviewed first

`PFTLFinalityVerifierV1` is bound (immutably) to the Arc-local
`SP1VerifierGateway` at `0x532D3a80…` rather than directly to the
`SP1VerifierGroth16` at `0xd3b199D0…`, and it forwards `proofBytes` without
pinning the four-byte selector. The gateway owner (`0xdB9b78C8…3814`) cannot
replace the registered `0x4388a21c` route (`RouteAlreadyExists`), but can
`freezeRoute` (halts egress) and can register a route under a *new* selector
to any contract exposing `VERIFIER_HASH()`; a "proof" carrying that selector
would then release funds. Reproduced by four tests in
`crates/ethereum-contracts/test/PFUSDCTier4.t.sol`
(`forge test --root crates/ethereum-contracts --match-contract PFUSDCTier4Test --match-test Gateway`).

Planned mitigation, to be delivered under this audit: bind `sp1Verifier`
directly to the Groth16 verifier and require
`bytes4(proofBytes[:4]) == 0x4388a21c` in the finality verifier; redeploy the
pair through the factory. The verifier is shared with the Ethereum mainnet
pfUSDC deployments, so the source change is scoped and reviewed rather than
patched in place.

## Evidence available now

- Arc testnet receipts for verifier/gateway/factory/probes, a real 1.000000 USDC
  deposit, and a locally verified Groth16 ingress proof:
  [evidence ledger](../evidence/arc-mvp-20260828/deployments.md),
  [benchmark](../evidence/arc-mvp-20260828/ingress-benchmark.md).
- Independent CI rebuild of both current guests, byte-identical ELFs and pinned
  vkeys: [reproduction manifest](../evidence/arc-mvp-20260828/program-reproduction.current-v2-docker-20260902.json).
- Six-validator PFTL devnet running the Arc-capable release after a
  deployment-exact six-clone gate:
  [gate and rollout receipt](../evidence/arc-mvp-20260828/devnet-20260902/gate-931-and-rollout-receipt.json).
- Conformance suites: `cargo test -p arc-conformance --locked`,
  `forge test --root crates/ethereum-contracts --match-contract PFUSDCTier4Test -vv`.

## Deliverables requested

Findings report with severity and reproduction; remediation re-review bound to
the fix commit; a short public statement of scope and result that we may link
from the post (wording controlled by Zellic).

## Contact and logistics

Post Fiat engineering will provide a walkthrough, a running devnet, the Arc
archive node used for registry proofs, and prover hardware for reproduction.
Commercial terms and the grant budget line for audits are founder decisions and
are not stated here.
