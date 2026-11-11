# Lane deploy sheet (Base Sepolia, chain id 84532)

| Role | Address | Nonce | Predicted contract |
|---|---|---|---|
| verifier deployer | `0x31f7bf1373d5376236534ab3cdb7af10c30b6c4c` | 0 | `0x305175cc93b8566ef506d7ef11b3a6de73777e1a` |
| lock deployer | `0x75031d303975d016f845d5f05a61c483c470f775` | 0 | `0x4cba0581d7fb8c789197db074826af27eeaef8f8` |

| Key | Gas on the fork | Test ETH (x3 margin) |
|---|---|---|
| verifier deployer | 2630563 | 0.000047350 |
| lock deployer | 1134485 | 0.000020421 |
| holder | 263732 | 0.000004747 |
| relayer | n/a | release gas cannot be estimated without a real proof; fund it separately |

Program: ELF sha256 `e92f20046c20761bb7a26ec47540000a88fad28b51874453437c3dd390f448ab`, key `0x000cef5bfa4586cfb79e255a950ae8c1dbb15c4270a968ad5bfcbe32d0a81414`, source commit `439803aaa3d6f34dfe97a4a48eca5d6008cba43d`.

Gas price read: 6000000 wei. SP1 verifier: `0xb69f2584cbcff99a58c4e7002e8b89af54a6f4e2` (V6_1_0_SP1_VERIFIER_GROTH16, https://github.com/succinctlabs/sp1-contracts/blob/v6.1.0/contracts/deployments/84532.json).

## Verifier constructor

- `sp1Verifier`: `0xb69f2584cbcff99a58c4e7002e8b89af54a6f4e2`
- `programVKey`: `0x000cef5bfa4586cfb79e255a950ae8c1dbb15c4270a968ad5bfcbe32d0a81414`
- `pftlChainIdHash`: `0x4ea8ee52b27fd187a726848ac2e0a41d4cf2c511922f08404f79d25fdbef878d`
- `pftlGenesisHashCommitment`: `0x813a05464157c7d51a427770c0b78ad126822e04601b895873a205c88e1421a3`
- `pftlProtocolVersion`: `1`
- `routeProfileHashCommitment`: `0x40c09fe4beb6697bdc8503fde412781521ba8755f43e17020ab02ae4323f6692`
- `routeEpoch`: `1`
- `assetIdCommitment`: `0xc538912614ef85f4e2578d85aa2000d7d408256f5135b3b6eac1012f272a65e5`
- `arbitrumChainId`: `84532`
- `vaultRuntimeCodeHash`: `0x83b160659967579c204911555c126fb7951843e737f63535932c290d3f413c20`
- `token`: `0x90ce303356d80ecbe359faf061c84130f40eacf0`
- `tokenRuntimeCodeHash`: `0x486d2a13ce1250e62b52acd787d2d2ff04aaf2d623f5159cb62ee55b7f1cb76b`
- `maxProofBytes`: `4096`
- `maxPublicValuesBytes`: `16384`
- `initialCheckpointCommitment`: `0x54bcb391734ea680047015da855694f09505c726bbc75637567a5e72390120be`
- `initialFinalizedHeight`: `1`
- `initialCommitteeRootCommitment`: `0xc93229a79c689c1688be4f4fc896bb01d322dfa17742fc69dfde999678b4025b`
