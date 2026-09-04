# Arc testnet deployments

Status: **G0 probes deployed; epoch-7 pair observed on chain; current-v2 pair
not deployed**. Chain ID 5,042,002 and the live receipts were rechecked on
2026-08-28 UTC.

!!! note "On-chain observation 2026-09-02"

    Read-only `cast` readback against `https://rpc.testnet.arc.io` shows the
    one-shot factory `0xcc8D866C…` reports `deployed() == true`, the predicted
    anchor `0x661D558a…` (6,171 runtime bytes) and vault `0xe88FB9ab…`
    (10,571 runtime bytes, `directIngress() == true`, owner `0xdB9b78C8…`)
    exist, the vault's `finalityVerifier()` is `0xC59EBED2A65B26e203f14C445b904dcf5F1B686b`
    (`programVKey() == 0x00c8d744…`, `routeEpoch() == 7`, `sp1Verifier()`
    = the Arc-local gateway), the anchor's `governedRouteBinding()` is
    `0x6edfa31c57cfeec8955572fd9cdb81b22222beb1dbac432ff7c7f0fc7ad9c520`, and
    the vault holds exactly 1,000,000 USDC atoms. The deployment transaction
    hashes were not recovered in this observation and the rows below remain
    unfilled until they are. That pair is pinned to the August egress vkey;
    the current-v2 release requires a fresh factory and pair pinned to egress
    vkey `0x0036cbe7…` and the PFTL height-931 checkpoint (see
    `docs/business/pfusdc-on-arc-20260902.md`).

At Arc block `59,330,485`, the common SP1 gateway address
`0x3B6041173B80e77f038f3f2C0f9744f04837185e` had no code. The Arc MVP
therefore deployed an Arc-local gateway rather than assuming a cross-chain
address.

## Fixed addresses

| Role | Address |
|---|---|
| Arc testnet USDC | `0x3600000000000000000000000000000000000000` |
| Operator/owner EOA | `0xdB9b78C87F76054b204188109b35cE4614d03814` |
| Arc-local SP1 gateway | `0x532D3a8035b87646a92245eCaa01e6602a13654e` |
| SP1 v6.1.0 Groth16 verifier | `0xd3b199D0C643dab3F28E5C97D0067763e28187b2` |
| Arc pfUSDC deployment factory | `0xcc8D866C40eBf78185B3f0ca8e540c3cc1411953` |
| Predicted ingress anchor | `0x661D558a818A07002C7D5da4A3179c4672FEf124` |
| Predicted vault | `0xe88FB9ab4890f513261F0aCA4FF13bfBa3e14862` |

## G0 probe sequence

The two testnet-only probes were deployed and exercised after funding:

```bash
export ARC_RPC=https://rpc.testnet.arc.network

forge create --rpc-url "$ARC_RPC" --private-key "$ARC_PRIVATE_KEY" \
  src/ArcConformanceProbe.sol:ArcConformanceProbe

forge create --rpc-url "$ARC_RPC" --private-key "$ARC_PRIVATE_KEY" \
  src/ArcUsdcPullProbe.sol:ArcUsdcPullProbe
```

Send transactions to all four `ArcConformanceProbe` methods and derive gas from their receipts. Then approve the pull probe for `1_000_000` USDC atoms and call `pull(USDC, 1_000_000)`. Record deployment hashes, call hashes, block hashes, status, gas used, effective gas price, and USDC before/after balances in the G0 evidence files.

Do not place the private key, shell history containing it, or raw signed transactions in this evidence directory.

## Production Arc pair

The final `PFTLFinalityVerifierV1`, route binding, and genuine PFTL checkpoint must be finalized before deploying the immutable pair. The deployment order is:

1. Deploy the SP1 v6.1.0 Groth16 verifier and Arc-local gateway, register selector `0x4388a21c`, and confirm the route is not frozen.
2. Deploy a fresh `ArcPfUsdcDeploymentFactory` from the owner EOA.
3. Read `predictedAnchor()` and `predictedVault()` and include them when deriving the Arc route/profile commitments. This must precede the immutable finality-verifier deployment.
4. Deploy `PFTLFinalityVerifierV1` with the exact fresh egress vkey, derived route/profile commitment, predicted vault runtime-code hash, and genuine PFTL checkpoint.
5. Call the one-shot factory `deploy(USDC, finalityVerifier, routeBinding, owner)`.
6. From the transaction receipt, record and cross-check the `ArcPfUsdcContractsDeployed` addresses.
7. Read back every immutable/storage binding and all runtime code hashes before sending USDC.
8. Verify the gateway, Groth16 verifier, factory, finality verifier, anchor, and vault sources on Arcscan.

The factory and pair are intentionally separate scripts because the predicted
vault address is an input to the governed route profile. From
`crates/ethereum-contracts`, first deploy the factory:

```bash
forge script script/DeployArcPfUsdcFactory.s.sol:DeployArcPfUsdcFactory \
  --rpc-url "$ARC_RPC" --broadcast
```

Finalize the PFTL route and checkpoint ceremony using that factory's
`predictedVault()` and `predictedAnchor()`, populate the non-secret variables in
`script/arc-pfusdc.env.example`. Resolve the immutable vault runtime hash in an
unbroadcast Arc fork simulation:

```bash
forge script \
  script/ComputeArcVaultRuntimeCodeHash.s.sol:ComputeArcVaultRuntimeCodeHash \
  --rpc-url "$ARC_RPC"
```

Copy the emitted hash to `ARC_VAULT_RUNTIME_CODE_HASH`, then run:

```bash
forge script script/DeployArcPfUsdcPair.s.sol:DeployArcPfUsdcPair \
  --rpc-url "$ARC_RPC" --broadcast
```

The pair script computes the route binding from the raw 48-byte profile hash,
derives all 48-byte-to-EVM commitments itself, checks the active SP1 route and
both verifier code hashes, and the supplied vault runtime code hash before
broadcasting the finality verifier and pair. The hash-probe script has no
broadcast region; it must not appear as an Arc transaction.

The official verifier source is pinned at `external/sp1-contracts` commit
`2ac5ecbbe473421a963d67e55f182e9a36576f7c` (tag `v6.1.0`). The repository's
SP1 SDK 6.3.1 reports circuit version `v6.1.0`; the generated Groth16 verifying
key SHA-256 and `VERIFIER_HASH()` are both
`0x4388a21c687fdd5f218d7e3d13190cac4c5355818d3605fd5fb811df468ee696`.
Older v1/v2 verifier sources are incompatible with proofs produced here.

Expected build readbacks (Solidity 0.8.20, optimizer 200 runs):

| Contract | Runtime bytes | Runtime code hash |
|---|---:|---|
| SP1 v6.1.0 Groth16 verifier | 6,741 | `0xc26a6452cb4fb09bc555e9ba44384da0267da540ec8700a87f8f4801520b2fa1` |
| SP1 verifier gateway | 1,931 | `0x028169f823c247e78b55e899ff3e88d87587acc97a9bbbd67ebd58bcf15ef491` |

Deploy the route from the compiler-isolated `crates/arc-sp1-contracts` package:

```bash
export ARC_RPC=https://rpc.testnet.arc.network
export SP1_GATEWAY_OWNER=0xdB9b78C87F76054b204188109b35cE4614d03814

forge script script/DeployArcSp1Verifier.s.sol:DeployArcSp1Verifier \
  --rpc-url "$ARC_RPC" --broadcast
```

Pass `PRIVATE_KEY` through the process environment; never write it into this
evidence directory or a command transcript. The route must remain active:
`freezeRoute(0x4388a21c)` is irreversible and makes the gateway reject these
proofs.

Required readback:

```text
factory.deployed()              == true
anchor.directIngress()          == true
anchor.bridge()                 == address(0)
anchor.l2Vault()                == vault
anchor.l2Token()                == USDC
anchor.l2ChainId()              == 5042002
anchor.governedRouteBinding()   == routeBinding
vault.directIngress()           == true
vault.arbSys()                  == address(0)
vault.ingressAnchor()           == anchor
vault.token()                   == USDC
vault.finalityVerifier()        == configured verifier
vault.owner()                   == configured owner
```

The SP1 route deployment completed at block 59,332,379 (block hash
`0x832a3ecf2208e1bcfade734382aabb2ef46a61aac9d2e60184d337f6af3814d5`).
Independent RPC readback returned:

```text
gateway.owner()                    = 0xdB9b78C87F76054b204188109b35cE4614d03814
gateway.routes(0x4388a21c)          = (0xd3b199D0C643dab3F28E5C97D0067763e28187b2, false)
verifier.VERIFIER_HASH()            = 0x4388a21c687fdd5f218d7e3d13190cac4c5355818d3605fd5fb811df468ee696
verifier.VERSION()                  = v6.1.0
gateway runtime bytes/code hash     = 1,931 / 0x028169f823c247e78b55e899ff3e88d87587acc97a9bbbd67ebd58bcf15ef491
verifier runtime bytes/code hash    = 6,741 / 0xc26a6452cb4fb09bc555e9ba44384da0267da540ec8700a87f8f4801520b2fa1
```

All three receipts have status `1`. Their combined gas cost was
43,261,470,000,000,000 native units, corresponding to approximately
0.04326147 system USDC at Arc's 18-decimal native/six-decimal ERC-20 mirror.

The one-shot factory was deployed in transaction
`0x3825cff995bf18be49d61cd62468ece3137248d52b4439c0af4f47c5648009db`
at block 59,332,489 (block hash
`0x73571712db9f7ac6833188e15df880f18d1853ad329cc0dca7192ad7c51ad608`).
The status is `1`, gas used is 2,375,856, and the effective gas price is
21,000,000,000. Readback shows the expected deployer, `deployed() == false`,
and the predicted anchor/vault addresses listed above. The runtime is 10,810
bytes with code hash
`0x609a7af5d468159741ea9571a1d01cbd783061371f8e960fe2f036ee0f1e0ad1`.

## Live evidence table

| Contract/action | Address | Transaction hash | Block | Status | Gas used | Source verified |
|---|---|---|---:|---|---:|---|
| `ArcConformanceProbe` deploy | `0x1766039cD6EeE04EF1ff2B3e882d222D78dbf793` | `0x7353c0108ed26999a03bc3e39c615fc9b775acc13381c90876f0d0b2ce5f2c36` | 59,331,639 | 1 | 333,518 | pending |
| `ArcUsdcPullProbe` deploy | `0xDd3B2cd9143b392c32A488607966423FFFEC9292` | `0xb8b598657e7e7d64e576eaf1a5ff2a4192caf037d58db3db7e5e0896f6ffef3a` | 59,331,643 | 1 | 254,658 | pending |
| SP1 v6.1.0 Groth16 verifier deploy | `0xd3b199D0C643dab3F28E5C97D0067763e28187b2` | `0x8265a88f196f88e6b40568462c13524e5436cb5490f6103d4a0c4f36b653c55c` | 59,332,379 | 1 | 1,510,757 | pending |
| Arc-local SP1 gateway deploy | `0x532D3a8035b87646a92245eCaa01e6602a13654e` | `0x6704c994abc397003dfdc7b31374a37ee48886e3aa84665a0e05517f8fe4f14c` | 59,332,379 | 1 | 498,546 | pending |
| register selector `0x4388a21c` | `0x532D3a8035b87646a92245eCaa01e6602a13654e` | `0xc604aa0fe82836d65f1b210de55342117ac594f81a49bc063c35247616b50107` | 59,332,379 | 1 | 50,767 | pending |
| `ArcPfUsdcDeploymentFactory` deploy | `0xcc8D866C40eBf78185B3f0ca8e540c3cc1411953` | `0x3825cff995bf18be49d61cd62468ece3137248d52b4439c0af4f47c5648009db` | 59,332,489 | 1 | 2,375,856 | pending |
| `PFTLFinalityVerifierV1` deploy | pending | pending | pending | pending | pending | pending |
| factory pair deployment | pending | pending | pending | pending | pending | pending |
| one-USDC vault deposit | pending | pending | pending | pending | pending | pending |
