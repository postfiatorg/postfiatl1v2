# Wallet / NAVSwap Agent Handoff - 2026-06-29

Generated at `2026-06-29T19:25Z` from local repo state after commits
`85623ac8` and `405a0b07`.

## Critical Truth

The live browser wallet does **not** currently implement an end-to-end
MetaMask Arbitrum USDC -> PFTL pfUSDC bridge-in flow.

There is a dormant helper file at `wallet-web/src/lib/evm.js`, but it is not
imported by any wallet UI component. No live wallet screen has a MetaMask
connect button, USDC approval button, Arbitrum bridge deposit form, deposit
event watcher, or PFTL pfUSDC claim flow.

Also, the dormant helper is incompatible with the current bridge contract:

- `wallet-web/src/lib/evm.js` encodes `deposit(uint256 amount)` with selector
  `0xb6b55f25`.
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol` exposes
  `deposit(uint256 amount, string calldata pftl_recipient, bytes32 nonce)`.
- `wallet-web/src/lib/utils.js` has `BRIDGE_VAULT_CONTRACT = ''`.

Do not tell the user that MetaMask USDC wrapping/bridging is wired in the
wallet. It is not.

## Current Services

Running locally:

| Service | URL | PID | Notes |
| --- | --- | ---: | --- |
| Wallet Vite server | `https://192.0.2.20:5173/` | `1863520` | HTTPS with temporary self-signed cert in `/tmp/postfiat-wallet-dev.*`. Browser may require accepting the cert. |
| Wallet proxy | `http://127.0.0.1:8080` | `1855258` | Proxies RPC and NAVSwap endpoints. |

Port `5173` is intentionally HTTPS now. Plain HTTP on the public IP disables
WebCrypto and breaks wallet encryption/import. The server was restarted with:

```bash
VITE_HTTPS_KEY=/tmp/postfiat-wallet-dev.key \
VITE_HTTPS_CERT=/tmp/postfiat-wallet-dev.crt \
npm --prefix wallet-web run dev -- --host 0.0.0.0
```

## Recent Commits

- `405a0b07 Fix wallet import flow`
  - Import flow now validates the 64-char seed first.
  - Passphrase fields appear only after seed validation.
  - `import-confirm` now calls the import handler, not the create handler.

- `85623ac8 Remove NAVSwap trustline gate`
  - Transparent NAVSwap no longer emits or accepts `trust_set`.
  - Swap UI no longer shows trustline controls for transparent NAVSwap.
  - Proxy readiness/funding no longer gates on recipient trustlines.
  - Validator execution allows incoming issued payments to existing accounts by
    implicitly creating the internal issued-balance record.

Previous commit `e5472794` added trustline preflight work and was superseded by
`85623ac8` for the transparent NAVSwap path.

## What Actually Works Now

### Wallet Loading

`https://192.0.2.20:5173/` returns `200 OK` locally via curl with a self-signed
certificate. Same-origin `/api/navswap/*` requests work through Vite's proxy.

### Wallet Import

The import screen now enforces this sequence:

1. Paste 64-char hex seed.
2. Click `Validate Seed`.
3. Confirm derived address.
4. Enter passphrase + confirmation.
5. Click `Confirm Import`.

This fixes the bad state where passphrase fields appeared before seed
validation and the UI could display `Passphrase must be at least 10 characters`
while still on the seed-validation screen.

### Transparent NAVSwap Readiness

Target wallet:

`pf124071fd53a12ca4556b7aa1f5ec98b585e73468`

Current readiness through HTTPS:

```json
{
  "ok": true,
  "status": "ready_to_submit_wallet_actions",
  "can_execute": true,
  "prepared_stages": ["nav_subscription_allocate"],
  "settlement_asset": {
    "balance_atoms": "6958370",
    "sufficient": true,
    "shortfall_atoms": "0"
  },
  "next_steps": ["submit the prepared wallet-owned actions"]
}
```

Guarded devnet pfUSDC funding was already submitted for this wallet:

`2f7970f89b52559fb96999cc69234035e5c0fcddecd0c31fac7f4235bbfb0aaacbe891402d4a7efc93fc17744156bc64`

The remaining transparent NAVSwap step is browser-local signing/submission of
the prepared `nav_subscription_allocate` action and observing operator
completion/a651 balance movement.

## What Does Not Work / Is Not Implemented

### MetaMask Arbitrum USDC -> pfUSDC

Not wired into the live wallet.

Current local code facts:

- `wallet-web/src/lib/evm.js` contains MetaMask helper functions:
  `connectMetaMask`, `ensureArbitrum`, `getUsdcBalance`, `approveUsdc`,
  `depositToBridge`, `usdcToAtoms`, `atomsToUsdc`.
- Nothing imports `wallet-web/src/lib/evm.js`.
- No React component exposes those functions.
- `depositToBridge` is wrong for the current contract ABI.
- `BRIDGE_VAULT_CONTRACT` is empty.
- There is no wallet-side flow to relay the Arbitrum deposit event into PFTL.
- There is no wallet-side flow to finalize/claim the corresponding pfUSDC on
  PFTL.

### Uniswap Atomic Handoff

Still disabled. The route is intentionally blocked unless bridge-aware wrapped
NAVCoin token, handoff controller, verifier mode, router, and new pool are
configured. Legacy a651/USDC pool is inspection-only and must not be presented
as the trustless PFTL handoff route.

## If The Next Agent Must Wire MetaMask Bridge-In

Do this explicitly; do not assume it already exists.

1. Confirm or deploy the Arbitrum `ERC20BridgeVault` for native USDC.
   - Contract source: `crates/ethereum-contracts/src/ERC20BridgeVault.sol`
   - Deploy script: `crates/ethereum-contracts/script/DeployERC20Bridge.s.sol`
   - Native Arbitrum USDC:
     `0xaf88d065e77c8cC2239327C5EDb3A432268e5831`

2. Configure wallet/proxy with the actual vault address.
   - `wallet-web/src/lib/utils.js` currently has `BRIDGE_VAULT_CONTRACT = ''`.
   - Prefer environment/config capability from proxy instead of hardcoding.

3. Replace the bad `depositToBridge` encoder.
   - Required Solidity signature:
     `deposit(uint256 amount, string calldata pftl_recipient, bytes32 nonce)`
   - The wallet must pass:
     - amount in USDC atoms, 6 decimals;
     - PFTL recipient address string;
     - fresh random `bytes32 nonce`.
   - Do not use the current `deposit(uint256)` selector.

4. Add wallet UI for:
   - Connect MetaMask.
   - Switch/add Arbitrum.
   - Show Arbitrum USDC balance.
   - Approve exact USDC amount to vault.
   - Deposit exact USDC amount to vault with PFTL recipient.
   - Display Arbitrum tx hash and deposit id/event status.

5. Add or find the relay path from Arbitrum event to PFTL pfUSDC.
   - The EVM deposit emits `ERC20BridgeDeposited`.
   - PFTL needs corresponding vault-bridge deposit evidence, attestation, and
     finalization/claim. Search the existing bridge scripts/tests before
     inventing a new protocol path.

6. Add tests before live use:
   - Unit test ABI encoding for the exact Solidity deposit signature.
   - Unit test MetaMask request sequence.
   - Integration/smoke test against a local mock `window.ethereum`.
   - If live, cap to a tiny amount and record Arbitrum tx + PFTL receipt.

## Verification Already Run

After `405a0b07`:

- `npm --prefix wallet-web test`: `117/117` passed.
- `npm --prefix wallet-web run build`: passed.
- `curl -skI --resolve 192.0.2.20:5173:127.0.0.1 https://192.0.2.20:5173/`:
  `200 OK`.
- `curl -sk https://127.0.0.1:5173/api/navswap/capabilities`: transparent
  stages are `nav_subscription_allocate`, `nav_redeem_at_nav`.
- `POST https://127.0.0.1:5173/api/navswap/readiness` for target wallet:
  `ready_to_submit_wallet_actions`.

After `85623ac8`:

- `cargo test -p postfiat-execution`: `75/75` passed.
- `node wallet-proxy/test_navswap_adapter.js`: passed.
- `npm --prefix wallet-web test`: `117/117` passed.
- `npm --prefix wallet-web run build`: passed.
- `node scripts/navswap-redaction-check.mjs`: passed.
- `git diff --check`: passed.

## Current Repo State

At handoff, `git status --short` was clean.

Relevant docs:

- `docs/plans/trustless-navswap-wallet-integration-spec.md`
- `docs/status/navswap-morning-handoff-2026-06-29.md`
- `docs/status/pfusdc-bridge-handoff-2026-06-19.md`
- `docs/status/arbitrum-contracts-code-review-2026-06-19.md`

Relevant files:

- `wallet-web/src/components/Onboard.jsx`
- `wallet-web/src/components/Swap.jsx`
- `wallet-web/src/lib/evm.js`
- `wallet-web/src/lib/utils.js`
- `wallet-proxy/server.js`
- `crates/ethereum-contracts/src/ERC20BridgeVault.sol`
- `crates/ethereum-contracts/script/DeployERC20Bridge.s.sol`
- `crates/execution/src/lib_parts/nft_escrow_asset_state_parts/part_03.rs`

## Warning For Next Agent

Do not claim the live wallet has end-to-end MetaMask Arbitrum USDC wrapping.
It does not. The transparent PFTL NAVSwap path is partially live and currently
ready for the target wallet to sign its prepared PFTL action; the EVM bridge-in
path is a separate missing integration.
