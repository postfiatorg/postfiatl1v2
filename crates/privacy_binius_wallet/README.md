# Recover and spend PFSH02 payments

Build with `cargo build --locked --release -p postfiat-node --features pq-wallet --bin pftl-pq-walletd`.
The Python client is `PYTHONPATH=python python3 -m postfiat_rpc pq-wallet`. Set `--wallet` to a
private directory and `--endpoint` to your trusted validator's RPC address before the command name.
The validator must have the M5 rehearsal amendment and private-action admission enabled.

1. Run `keygen --key-file /secure/wrapping.key` once. In a second terminal run
   `daemon --binary /path/to/pftl-pq-walletd --key-file /secure/wrapping.key`. Keep the key
   outside the wallet and back it up independently. The daemon creates the owner-only wallet
   directory and Unix socket. Each daemon runs one job at a time.
2. Run `init --account 0 --first 0 --count 2`, then `backup --file /secure/birthday.backup`.
   The encrypted v1 recovery record contains a random 48-byte seed, network, account, address
   range and finalized birthday checkpoint. Use `address --index 1` to obtain a shareable
   receive address. Use a fresh seed after a seed disclosure.
3. Run `shield --asset PFT --amount 100 --fee 1 --payer ACCOUNT --account-key /secure/account.json
   --sequence NEXT`. Amounts and fees are integer atoms. The wallet checks the named public
   payer against the local ML-DSA-65 account key file. For pfUSDC use the exact governed
   source-series ID and supply the public PFT fee payer, signing key and next sequence for all modes.
4. Run `pay --asset PFT --amount 30 --fee 1 --to RECEIVE_ADDRESS`, or `unshield --asset PFT
   --amount 18 --fee 1 --to PUBLIC_ACCOUNT`. The client prints the fee source before proving.
   It returns the intent ID, proof time and prover peak RSS. `receipt INTENT_ID` reads admission
   and inclusion status. Run `sync` to recover finalized notes, then `balance` to see per-issue
   finalized atoms and the separate pending change. `status` shows the current job and elapsed time.
5. Use `cancel` to interrupt a job. An interrupted submission can already have reached admission.
   Run `sync`, then `submit-pending` to retry the saved action if still pending. One outstanding
   action per wallet prevents spending the same notes while its outcome is unknown.
6. In a fresh wallet directory, start the daemon with the original wrapping key, run
   `restore --file /secure/birthday.backup` and `sync`. Recovered witnesses use the birthday
   frontier plus public commitments through the latest finalized eligible root, including the
   terminal root of a closed epoch. To combine funds across epochs, first pay yourself from an
   older epoch and sync after finality.

## Derivation and encodings

HKDF-Extract uses HMAC-SHA-384 with the ASCII salt `postfiat/pq-wallet/v1/hkdf-salt` and the
48-byte seed as IKM. HKDF-Expand uses the RFC 5869 block counter starting at one. Info is the
concatenation of five fields, each preceded by its **four-byte big-endian byte length**, in order.

| Field | Bytes |
| --- | --- |
| Role | ASCII `postfiat/pq-wallet/v1/spend` or `postfiat/pq-wallet/v1/kem` |
| Network | UTF-8 chain ID |
| Genesis | 48 raw genesis-hash bytes |
| Account index | Four-byte unsigned big-endian integer |
| Address index | Four-byte unsigned big-endian integer |

The first label yields 48 bytes for S. The second yields 64 bytes, with ML-KEM-1024 d followed by z.
The circuit derives the serial from S and the coin. Envelope encryption uses the existing
`PFT-SH-KEM-KEY-V2` and `PFT-SH-KEM-NONCE-V2` HMAC-SHA-384 domains. Account authorization
uses the separately supplied account key. The address is ASCII `pfpq1` followed by unpadded
base64url of version byte 1, SHA-384 of the two framed network fields, the 48-byte owner
commitment, the 1,568-byte ML-KEM encapsulation key, and the first 12 bytes of SHA-384 of all
preceding binary address fields. Public interoperability vectors are in [vectors](vectors/).

The wrapping key encrypts wallet state and backups with AES-256-GCM and fresh 96-bit nonces.
The two file roles use distinct versioned AAD. Restore needs both backup and wrapping key.
Use a trusted validator for checkpoint identity and public state. With a separate archival page server,
set `--validator-endpoint` to your independently operated validator. The scanner checks the
certified block reference, exact pair-chain coverage and locally reconstructed tree roots
before publishing a checkpoint. A transport interruption or expired cursor leaves the
previous authenticated checkpoint available for resumption.

## Open the browser wallet

Build wallet-web with `npm ci && npm run build` in `wallet-web/`. Start and initialize the native
wallet as above. On the gateway, configure the existing private-action validator sources, the Orchard
asset allowlist and `WALLET_PROXY_PQ_ISSUE_ID`. Allow the exact local origin (by default
`http://127.0.0.1:8790`) for the wallet’s authenticated principal. The separate PQ HTTP routes live
under `/api/private/v1/pq/`. The original Orchard routes remain under `/api/private/v1/`.

On your device, run the following with the public account, exact source-series ID and appropriate paths.

```sh
PYTHONPATH=python python3 -m postfiat_rpc pq-wallet --wallet "$wallet_dir" --endpoint "$validator" browser \
  --dist wallet-web/dist --gateway "$gateway_origin" --gateway-token-file "$gateway_credential_path" \
  --payer "$public_account" --account-key "$signing_key_path" --issue "$source_series_id" \
  --pair-file "$wallet_dir/browser.link"
```

Open the URL in the owner-only pairing file on the same device. Treat that link as authorization to
use the wallet. The bridge serves the bundled wallet-web UI on loopback. It checks the HTTP host,
origin and a process-specific session capability on every API request. Stop the bridge to revoke the
session. If an abrupt shutdown leaves an old pairing file, restart with a fresh pairing-file path.
Each user runs their own wallet directory, daemon and bridge. Choose a separate port with
`--port` for a second user on the same machine.

**Sync** downloads public pages and checks the reconstructed wallet checkpoint. **Receive address**
shows the shareable PQ address. Choose **Shield**, **Pay** or **Unshield**, enter integer atoms, review
the exact issue, recipient and fee source, then confirm. The daemon proves and signs locally. The UI
checks the signed public terms in WASM before submitting the public action to the gateway. **Check
finality** compares the gateway receipt with the corresponding certified block at your trusted
validator. A pending change becomes spendable after a finalized sync.

For migration, prepare a private-egress batch with the existing Orchard wallet. Pass its public batch
file via `--orchard-exit-batch` when starting the bridge. The exit must pay the configured account
in the selected issue. Review its public amount, fee, policy and disclosure, and authorize the exit
in **Orchard migration**. Check its receipt, then separately review and approve the new PQ shield.
Restarting either service preserves both intent IDs in the owner-local journal. A missing or
conflicting receipt leaves the migration waiting for confirmation. The two public amounts, account
and timing can link these transactions.
