# A666 unattended Ethereum export relay

This deployment lets the localhost browser wallet deliver newly issued native A666 to the connected MetaMask account as mainnet `wA666` without a shell harness or operator action per user.

## Production identity

- PFTL route: `pftl-a666-ethereum-wA666-usdc-v1`
- Route digest: `12ed00ca87e29554ce4b978da1710fffc0830767e84e62f08df257f727db953efdd89bcf6ea99f5634d6e5ea8aca2933`
- Ethereum controller: `0x9A0262C0572fb4DB08765408eB225E207F40c3d9`
- Ethereum token: `0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5`
- Proof verifier: `0xb79FF97EcC11574a8A78d0b5a9D7C8c2A94bF96A`
- SP1 program vkey: `0x004e44aca326861252ee5ff7863b1174635b727759b75d46b28bb28d4a7b34f9`

## User flow

1. The browser checks route, PFTL invariant, Ethereum contracts, prover, and signer readiness.
2. It prepares the exact proof-bound Ethereum mint packet locally.
3. Before signing a value-moving PFTL operation, it creates a durable relay job bound to packet hash, packet digest, recipient, amount, and deadline.
4. The worker waits for the exact finalized PFTL source debit.
5. It proves the PFTL receipt on the pinned A100 prover, accepts the proof and mints the exact `wA666` packet on Ethereum, waits for Ethereum finality, then records PFTL destination consumption.
6. The browser reports success only after the durable job is accepted and the recipient's ERC-20 balance has increased by the requested amount.

Jobs are idempotent by packet hash. The supervisor permits one active export, orders discovered packets by PFTL source height, uses atomic state files, rejects PID reuse, applies bounded retry backoff, and resumes from proof/mint/ack evidence after a service or host interruption.

## Services and readiness

The enabled `pft-wallet-proxy-8080.service` owns the durable worker; its installed unit matches `pft-wallet-proxy-8080.service` in this directory. Caddy uses `Caddyfile.localhost-production`, serves `wallet-web/dist` directly through localhost TLS on port 5173, and proxies only the bounded current RPC/API routes to the wallet proxy. Retired Arbitrum/CCTP browser proxies are absent. The former Vite preview service is disabled; it is not a production dependency. The Caddy container uses an `unless-stopped` restart policy.

After a reviewed browser change, run `npm run build` in `wallet-web`; Caddy's read-only bind mount exposes the resulting immutable bundle without a development server.

Readiness:

```bash
curl -ksS https://127.0.0.1:5173/api/a666/export-readiness | jq .
```

`ready:true` requires the live PFTL invariant, correct Ethereum chain/contracts/vkey, unpaused minting, pinned remote prover binary and ELF, and an unlocked StakeHub signer whose policy permits the verifier and controller.

The enabled signer unit matches `stakehub-pfusdc-wallet-agent.service` in this directory. The StakeHub signer intentionally fails closed after an agent restart. An operator must unlock it; the relay never stores the vault passphrase. This is an availability dependency, not a user-custody dependency. Readiness blocks all PFTL value movement before a user begins if the signer is locked.

## Recovery

Inspect jobs without exposing custody material:

```bash
find /home/postfiat/.local/state/postfiat-a666-wallet/a666-export-jobs/jobs \
  -mindepth 2 -maxdepth 2 -name worker-state.json -print
```

Restarting the proxy is safe:

```bash
systemctl --user restart pft-wallet-proxy-8080.service
```

Do not delete or edit a live job directory. The worker resumes from its durable artifacts. A terminal `failed` state means a binding/safety gate failed and requires diagnosis; `retry_wait` means the job will retry automatically.
