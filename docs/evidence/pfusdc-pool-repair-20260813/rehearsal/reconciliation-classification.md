# pfUSDC height-902 reconciliation classification

Snapshot: finalized PFTL height `902`

State root: `78dd159fca28a6648acd32a74c720cef045bedd4ca094eff3d1c969c04a34c48dd3e9f9dcd29e9f7a944dcc0c63eaad7`

Asset family: `02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b`

This report classifies the `105.357569 USDC` positive residual emitted by
`conservation-audit.json`. It does not call the legacy pooled ticker par. The
new source-series rules keep these legacy amounts separate from epoch-6
issuance.

## Aggregate equation

All values are six-decimal USDC atoms.

```text
source vault balances                                      420.990527
- quarantined/unallocated mainnet epoch-3 surplus          100.000000
- retired Arbitrum route surplus                             5.000010
- non-production Sepolia test-token balance                  0.357559
---------------------------------------------------------------
liability-bearing source dollars                           315.632958

live PFTL bridge claims                                    289.700585
+ finalized deposit awaiting claim                          15.000000
+ burned, not PFTL-settled redemptions                      37.932373
- source-paid, not PFTL-settled redemptions                 27.000000
---------------------------------------------------------------
expected liability-bearing source dollars                 315.632958
```

The residual is therefore fully classified:

```text
100.000000 + 5.000010 + 0.357559 = 105.357569
```

It is a positive surplus/classification issue, not missing cash. None of it is
counted as backing for new epoch-6 source-series pfUSDC.

## Route and bucket classifications

### Arbitrum epoch 1 — retired, surplus quarantined

- Vault: `0x850e4ceea147f3551c68c2251129e5945d0afb58`
- Source balance: `6.000020`
- Recorded bucket claims: `1.000010` (`1.000000` outstanding plus `0.000010` pending redemption)
- Historical deposits: `2.900120`
- Historical redemptions: `1.900120`
- Classification: `1.000010` liability-bearing; `5.000010` retired-route surplus.

The ten-atom pending burn at height 85 also explains the exact difference
between family issued supply (`289.700595`) and bridge outstanding supply
(`289.700585`). It remains part of the explicitly legacy-pooled accounting
domain and is not attributed to epoch 6.

### Sepolia epoch 2 — excluded test asset

- Vault: `0xebacc5b43351f18ff605586afc7ddabc2ca09dff`
- Balance: `0.357559` Sepolia test USDC
- PFTL deposit records/bucket claims: zero
- Classification: non-production test asset; excluded from mainnet USDC backing.

### Mainnet epoch 3 — unallocated vault surplus

- Vault: `0x47d54874a708c4bf25ffd547f61f695fff940af9`
- Balance: `100.000000`
- PFTL deposit records/bucket claims: zero
- Classification: proven on-chain mainnet USDC, but not allocated by a
  proof-bound PFTL receipt. Quarantined until a proof-bound migration or return
  under the deployed contract's actual rules.

### Mainnet epoch 4 — exact after source-paid withdrawals

- Vault: `0x8583409ddbac984ec195dfa06a21103d92403c1e`
- Vault balance: `104.520000`
- Outstanding claims: `103.520500`
- Redemption queue: `27.999500`
- Source-paid but PFTL-unsettled: `27.000000`

```text
104.520000 + 27.000000 = 103.520500 + 27.999500 = 131.520000
```

Classification: exact; the PFTL settlement records must catch up to already
proved source releases.

### Mainnet epoch 5 — cash exists, claims impaired because proof path is dead

- Vault: `0xaaa78fda7062efce769e95cd72fc55e507bc8183`
- Vault balance and contract obligations: `195.031396`
- Outstanding claims: `185.098533`
- Pending redemption: `9.932863`
- Bucket factor/status: `0 bps`, `impaired`

The deployed vault has no upgrade, verifier-setter, rescue, or sweep path. Its
immutable verifier accepts the obsolete epoch-5 program and cannot authenticate
the later PFTL burn. A new Ethereum contract cannot pull these dollars from the
old vault. The cash is physically present but not presently redeemable under
the deployed cryptographic rules; the wallet must not value this series at par.

### Mainnet epoch 6 — exact, including the user's pending claim

- Vault: `0x4939a45caa85da31fb26d7dbe6477b45f7f08688`
- Vault balance: `15.081552`
- Existing live bucket claim: `0.081552`
- Finalized unclaimed deposit: `15.000000`

```text
15.081552 = 0.081552 + 15.000000
```

Classification: exact healthy successor backing. The repair mints the pending
`15.000000` into the epoch-6 source-series asset, never into the legacy pooled
ticker.

## Allocation-ledger correction

Historical settlement reduced bucket liabilities but did not retire the
matching immutable supply allocations. Before source-series issuance, the new
transition deterministically retires the oldest remaining supply allocations
within each bucket until:

```text
remaining supply allocations = outstanding bridge supply + redemption queue
```

The height-902 corrections are:

| Bucket | Recorded remaining | Required remaining | Retire |
|---|---:|---:|---:|
| Arbitrum epoch 1 | `2.900090` | `1.000010` | `1.900080` |
| Mainnet epoch 4 | `131.520000` | `131.520000` | `0` |
| Mainnet epoch 5 | `290.690781` | `195.031396` | `95.659385` |
| Mainnet epoch 6 | `80.000000` | `0.081552` | `79.918448` |

This correction changes allocation metadata only. It does not mint, burn,
move, revalue, or reassign any holder asset. Subsequent source-series
redemption settlement retires the matching allocation in the same atomic
transition.

## Decision

- New epoch-6 issuance may proceed only as the exact epoch-6 source series.
- The current pooled asset remains labeled and valued as `legacy pooled`, not
  as successor-vault dollars.
- The `105.357569` residual remains quarantined/excluded until a proof-bound
  migration or source return.
- The epoch-5 factor remains zero unless real recapitalization or a valid
  cryptographic recovery path is proved.
