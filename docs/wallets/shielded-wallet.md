# Shielded Wallet

The existing Orchard wallet and the PFSH02 rehearsal wallet use separate keys and scan different pools.

## PFSH02 browser wallet

On the local rehearsal chain, two wallets can shield PFT or the enabled pfUSDC source series, pay a
recipient and withdraw to a public account. Build `wallet-web/dist` and initialize your local
`pftl-pq-walletd` wallet using the instructions in `crates/privacy_binius_wallet/README.md`.
Start the browser interface on the same device.

```sh
PYTHONPATH=python python3 -m postfiat_rpc pq-wallet --wallet "$wallet_dir" --endpoint "$validator" browser \
  --dist wallet-web/dist --gateway "$gateway_origin" --gateway-token-file "$gateway_token_path" \
  --payer "$public_account" --account-key "$key_path" --issue "$source_series_id" \
  --pair-file "$wallet_dir/browser.link"
```

Open the link in the pairing file, select **Sync**, then choose **Shield**, **Pay** or **Unshield**.
The signer review shows the issue, amount, recipient and PFT fee source. The daemon proves and signs
on your device. **Check finality** verifies the receipt against your trusted validator. The privacy
panel counts recent payments for the selected issue and action. The gateway can observe the caller and timing.

To migrate a policy-permitted issued-asset note, supply the existing Orchard wallet’s public
private-egress batch with `--orchard-exit-batch`. Authorize its exit, check finality, then approve
a separate PQ shield. The public account and amounts link the two transactions. The wallet remembers
the exit across restarts and waits for a fresh shield approval.

## Orchard commands

- `orchard-keygen`;
- `orchard-view-key-export`;
- `orchard-scan`;
- `orchard-disclose`;
- `orchard-disclosure-verify`;
- `orchard-deposit-create`;
- `shield-batch-orchard-deposit`;
- `orchard-spend-create`;
- `orchard-withdraw-create`;
- `shield-batch-orchard-withdraw`.

## Orchard flow

1. create or import Orchard keys locally;
2. deposit transparent value into the Orchard pool;
3. scan for decrypted outputs;
4. spend or withdraw from retained witness material;
5. disclose selected facts when needed.

## Source

- `crates/node/src/privacy.rs`
- `crates/privacy_orchard/src/lib.rs`
- `docs/status/privacy-production-burndown.md`
