# Chain-bound open reserve-proof qualification

**Date:** 2026-08-01  
**Result:** PASS under the path-independent, reproducibly rebuilt guest  
**Trust class:** CONTROLLED; qualification only; no live value

This is the corrected end-to-end qualification of the public reserve-proof
kit. Unlike the superseded first run, every consensus identity is derivable
and usable:

- PFTL genesis: WAN-devnet-2
  `ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9`;
- deterministic qualification issuer:
  `pf0fae169e4293feebc8c9119febb4fd995a667b37`;
- issued qNAV asset ID:
  `3f631473a34a48cd47b4e1067546a9ccc5fcfe2f6e103655191d600d9574a5b2e6a985b7c52dcff7c9461aac872a12f5`;
- source-manifest hash:
  `9da4e2ba55939f138475026946d2728d9b40d3f4c7762289a70aae94584eac924b9a788c6df25c9276cc83f1616ef0e5`;
- consensus-derived successor profile ID:
  `3d78cac1f539d3d2e56f6f38c958242aa0bcd13661c733834896bc9c49a48211d716bd4cad83d478b2fa5d85b22a0c7e`;
- guest ELF SHA-256:
  `0f8476431677bfe0a8f9f19db7439abce1a879ba5736cfa3225ae7de4e5b0e52`;
- SP1 program vkey:
  `0x000c7271e0711abce0c61d293222fd4a144599a779db8cadadc4df35e31a4100`.

The artifact hashes below bind the current guest identity. The CPU proof was
independently verified by the host verifier and by the execution-layer
consensus verifier; a one-byte proof mutation was rejected.

The issuer identity is deterministically reproducible from the public
qualification-only master seed `66` repeated 32 bytes, account index zero,
and chain ID `postfiat-wan-devnet-2`. It must never control live value.

## Reproducible identity correction

The first chain-bound run used a native `cargo prove build`. Remote CI exposed
that its ELF embedded the host checkout path, so a different checkout could
produce a different vkey from identical source. That candidate identity was
never registered or activated. The qualification was therefore rerun under
the pinned SP1 6.3.1 Docker image with the repository mounted through
`--workspace-directory`.

Docker builds from two distinct host checkout paths produced the exact same
ELF SHA-256 and program vkey shown above. The committed ELF, profile,
public-value fixture, and real Groth16 proof were all regenerated from that
canonical build. Native builds remain valid development checks but cannot
define a production proof identity.

## Results

- `profile derive` produced the exact profile ID above using the same
  `NavProfileRegisterOperation::to_profile()` implementation as consensus.
- `observe` accepted two strictly ordered controlled sources.
- witness construction emitted 3,515 bounded CBOR bytes.
- native and SP1 execution emitted identical 584-byte public values.
- CPU Groth16 proving completed successfully.
- an independent verifier accepted the proof and decoded the exact chain,
  asset, profile, manifest, policy, interval, totals, and trust classes.
- `packet build` emitted a valid 356-byte proof-calldata operation for the
  deterministic issuer and qNAV asset.
- changing serialized proof byte 100 from decimal 49 to `0x37` caused a
  deterministic Groth16 verification failure.

The qualified totals are gross assets 1,400, liabilities 300, and net assets
1,100. All 1,100 units are explicitly `CONTROLLED`; the profile opts into
controlled evidence solely for this zero-value qualification asset.

## Artifact hashes

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `witness.cbor` | 3,511 | `38a43f312b578b323b4356da008afa147c913667b6911277de0549a58c19c306` |
| `proof.bin` | 2,277 | `4bb161cebd26ffbbf7db8326d836570ba4d7cf90a1fdba4ea7171bb61ea742ea` |
| `proof-calldata.bin` | 356 | `fe3378eee7d39e8090efe3dab9e1a68474a3a72678d47a1a270c94ad6030d188` |
| `public-values.bin` | 584 | `95bc0bc04ddb66dac961911754111bf0fc4f56f6d4641f75991d6003d3a16d64` |
| `proof-report.json` | 191 | `08a414831734ee24684b38d62f03585102744aa80c45bdb3bb9c07ba30b22f44` |
| packet operation | 8,568 | `bc3703f29d0ef90ea726ff4ec255fc2d6e758656cd32ce14161598e335615cc2` |

The consensus-facing calldata and public values are committed as lowercase
hex fixtures under `crates/execution/testdata` and are verified by the L1 test
suite. The larger serialized host proof is not a consensus input.

## Consensus and lifecycle follow-up

The provider-neutral qNAV identity above was also exercised through the
consensus implementation on 2026-08-01:

- `provider_neutral_qnav_proof_finalizes_and_survives_six_validator_restart`
  finalized the registered profile and reserve packet on six validators,
  checked one state root, restarted the fleet from durable state, and checked
  the same finalized proof state again;
- `pftl_uniswap_consensus_subscribe_export_and_refund_moves_real_balances`
  exercised transparent issue and redeem, private-middle issue and redeem,
  proof-bound export and destination consumption, return import, conservation,
  replay rejection, and tamper rejection using qNAV's provider-neutral proof;
  and
- the generic export/return supervisor suites exercised idempotency, restart,
  wrong-route rejection, and simultaneous distinct-route isolation.

These are controlled, zero-value qualification results. They prove that the
provider-neutral protocol path is not A666-specific. They do not claim that an
unaffiliated operator independently reproduced the run, that the qNAV wrapper
was deployed to Ethereum mainnet, or that live A666 governance has activated
the successor profile.
