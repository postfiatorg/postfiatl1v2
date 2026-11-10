# Lane deploy sheet (Base Sepolia, chain id 84532)

| Role | Address | Nonce | Predicted contract |
|---|---|---|---|
| verifier deployer | `0xf1df66c6395ec5be01af34897fc32e83ae3d62d2` | 0 | `0x3ef8a003e9ef4638c5882bf0976a6df039b4300d` |
| lock deployer | `0x5d1c9b41d55406e4313a6cbe9203c97a3c75b5be` | 0 | `0x0c388ce7118c092e5c310b932fbf3ae6fb9613cc` |

| Key | Gas on the fork | Test ETH (x3 margin) |
|---|---|---|
| verifier deployer | 2630551 | 0.000047350 |
| lock deployer | 1134485 | 0.000020421 |
| holder | 263732 | 0.000004747 |
| relayer | n/a | release gas cannot be estimated without a real proof; fund it separately |

Program: ELF sha256 `e92f20046c20761bb7a26ec47540000a88fad28b51874453437c3dd390f448ab`, key `0x000cef5bfa4586cfb79e255a950ae8c1dbb15c4270a968ad5bfcbe32d0a81414`, source commit `439803aaa3d6f34dfe97a4a48eca5d6008cba43d`.

Gas price read: 6000000 wei. SP1 verifier: `0xb69f2584cbcff99a58c4e7002e8b89af54a6f4e2` (V6_1_0_SP1_VERIFIER_GROTH16, https://github.com/succinctlabs/sp1-contracts/blob/v6.1.0/contracts/deployments/84532.json).

## Verifier constructor

- `sp1Verifier`: `0xb69f2584cbcff99a58c4e7002e8b89af54a6f4e2`
- `programVKey`: `0x000cef5bfa4586cfb79e255a950ae8c1dbb15c4270a968ad5bfcbe32d0a81414`
- `pftlChainIdHash`: `0x4ea8ee52b27fd187a726848ac2e0a41d4cf2c511922f08404f79d25fdbef878d`
- `pftlGenesisHashCommitment`: `0x00adf8ea5f9dd5f840fcd028b5830b945b0cdd0b03523af6afad99ae2ea77fbd`
- `pftlProtocolVersion`: `1`
- `routeProfileHashCommitment`: `0xbc08cd934b92866c544c4c29a605ace538bd96ceb071b74fa14f7d85d670e0b9`
- `routeEpoch`: `1`
- `assetIdCommitment`: `0xef6331cbf5e3c10da961e747bd1662a4b53ca13b12225d384eb5d97a859c1998`
- `arbitrumChainId`: `84532`
- `vaultRuntimeCodeHash`: `0xceed4e9265b55fe1c89b44d9384385f1c19a9d884b9fa04fb82a80c7192b0cb6`
- `token`: `0x90ce303356d80ecbe359faf061c84130f40eacf0`
- `tokenRuntimeCodeHash`: `0x486d2a13ce1250e62b52acd787d2d2ff04aaf2d623f5159cb62ee55b7f1cb76b`
- `maxProofBytes`: `4096`
- `maxPublicValuesBytes`: `16384`
- `initialCheckpointCommitment`: `0xbbf518e84d8a52eb871b5182b8b13b6ae104582ac3f35977e21a0088f07529a7`
- `initialFinalizedHeight`: `1`
- `initialCommitteeRootCommitment`: `0x80842b28e842cbfc493414d919db0f9f3e48b766f35e92b7b79e1d15ad34a7f3`
