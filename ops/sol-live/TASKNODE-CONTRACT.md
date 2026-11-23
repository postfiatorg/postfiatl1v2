# Solana USDC withdrawals

Relay upstream is loopback HTTP on port 18085. Keep the existing Ethereum upstream and wallet storage key. Use a Solana-specific tracking key containing network, wallet account and address. Responses use JSON.

* GET /solana/withdrawals/readiness returns 200. An unavailable route returns {"ok":true,"ready":false,"message":"reason"}. Enable Review only when ready is true. A ready response also contains asset_id (family), series_asset_id, bucket_id, issuer, epoch (current finalized reserve epoch, integer), reserve_packet_hash, bucket_outstanding_atoms (decimal string), current_height, destination_prefix, vault_address, relayer_lamports and relayer_floor_lamports. Destination prefix includes the trailing colon.
* POST /solana/withdrawals accepts application/json with exactly {"burn_tx_id":"<96 lowercase hex digits>"}. Maximum body is 256 bytes. Returns the job (202 queued or in progress, 200 paid). Repeating a request tracks the same burn. A retry of a failed job returns it to queued. A failure never asks the holder to sign a second burn.
* GET /solana/withdrawals/<burn_tx_id> returns 200 with the job. Stages are queued, reading, proving, paying, paid and failed. Fields are burn_tx_id, stage, detail, error, amount_atoms (decimal string), recipient (base58 owner), burn_height and solana_tx (base58 payout signature). Unknown values are null. Show error on failure and link solana_tx to the mainnet explorer when paid.
* Errors return {"error":"reason"}. Codes are 400 for malformed input or ID, 404 for an unknown job or path, 411 for missing length, 413 for oversized input, 415 for the wrong media type and 429 for a full queue.

## Wallet signature

Refresh readiness before quoting. Match its series_asset_id against the user's account_assets balance line and verify its source_bucket_id when returned. Offer only the matched spendable amount. Refuse zero, over-balance, over-outstanding and values beyond the wallet's exact integer range. USDC uses six decimal places. Never sum Ethereum or earlier Solana series into this limit.

Use a canonical base58 public key that decodes to exactly 32 bytes. Hex-encode all 32 bytes in lowercase including leading zeroes. Append that to the supplied prefix (mainnet is solana-spl:45296998a6f8e2a784db5d9f95e18fc23f70441a1039446801089879b08c7ef0:). The address is the receiving wallet owner. The payout worker derives its associated token account.

Sign this single intent with client.quote, client.sign and client.submit, replacing the typed placeholders with fresh values.

```json
{"kind":"asset","operation":{"operation":"vault_bridge_burn_to_redeem","owner":"<client.address>","issuer":"<issuer>","asset_id":"<asset_id>","bucket_id":"<bucket_id>","amount_atoms":20000,"epoch":1,"reserve_packet_hash":"<reserve_packet_hash>","destination_ref":"<destination_prefix><owner hex>"}}
```

Here 20000 is 0.02 pfUSDC. Replace the example epoch with readiness.epoch. Persist result.txId immediately and POST it. Poll GET roughly every eight seconds. Restore tracking after reload. The Solana recipient supplies no signature.
