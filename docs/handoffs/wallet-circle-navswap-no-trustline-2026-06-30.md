# Wallet Circle Bridge and No-Trustline NAVSwap Handoff

Date: 2026-06-30 UTC  
Hosted wallet: https://192.0.2.20:5173/
Repo: `$POSTFIAT_REPO`

## Current State

The hosted wallet is live with:

- Circle CCTP v2 fast bridging for Ethereum -> Arbitrum USDC. Do not route this through LI.FI.
- Arbitrum ETH gas UX for showing gas state and guiding the user to bridge/add gas.
- Transparent NAVSwap `pfUSDC -> a651` wired through the wallet.
- No user-facing or API-level NAVSwap trustline requirement.

The live public bundle was fetched from the hosted URL and verified:

- HTML status: `200`
- Current JS bundle: `/assets/index-CVmp8JIp.js`
- Current CSS bundle: `/assets/index-BT0tBap2.css`
- Fetched JS/CSS contained no `trustline`, `trust line`, or `trust_set` strings.
- Fetched JS contained the vault allowance selector `0xdd62ed3e` and vault error selector `0xbe24f3c5`.
- Fetched JS contained no stale bad vault deposit selector `0x6c7eca6d`.
- Fetched JS contains the redesigned Bridge screen and the `Where is my USDC?` location panel.
- `/api/navswap/capabilities` advertises transparent wallet stages:
  - `nav_subscription_allocate`
  - `nav_redeem_at_nav`
- Direct `POST /api/navswap/actions/prepare` with `stage: "trust_set"` returns HTTP `409` with code `transparent_navswap_trust_set_not_supported`.

## What Changed

### Protocol

Incoming issued-asset credits no longer require a user-created trustline row for the covered receive/mint paths. The execution layer now creates implicit issued balance rows with `reserve_paid = 0`, `authorized = true`, and `frozen = false`.

Important files:

- `crates/execution/src/lib_parts/nft_escrow_asset_state_parts/part_03.rs`
  - Added shared implicit issued-credit helpers.
  - `issued_payment` and shielded issued-asset egress now auto-create or expand recipient rows.
- `crates/execution/src/lib_parts/nft_escrow_asset_state_parts/part_01.rs`
  - `NavMintAtNav` now credits recipients through the implicit issued-credit helper.
- `crates/execution/src/lib_parts/nft_escrow_asset_state_parts/part_02.rs`
  - NAV redeem settlement, vault bridge deposit claim, and vault bridge mint-from-receipts now use the same implicit credit path.

Generic trustline/accounting structures remain in the protocol for spend, reserve, offer, escrow, and non-NAVSwap flows. Do not delete them blindly. The requirement removed here is the user/NAVSwap receive-side requirement.

### Wallet Proxy

Important files:

- `wallet-proxy/server.js`
  - Transparent NAVSwap trust_set preparation is rejected with `transparent_navswap_trust_set_not_supported`.
  - Capabilities only expose `nav_subscription_allocate` and `nav_redeem_at_nav`.
  - Guarded devnet pfUSDC funding is enabled in the hosted compose path.
- `wallet-proxy/test_navswap_adapter.js`
  - Tests assert trust_set is rejected and no trustline readiness fields are required.
- `docker-compose.wallet.yml`
  - Mounts the release `postfiat-node` binary into wallet proxy.
  - Enables `NAVSWAP_ENABLE_DEVNET_PFUSDC_FUNDING=true`.

### Wallet Web

Important files:

- `wallet-web/src/lib/navswap-actions.js`
  - Browser-side NAVSwap signing helper now rejects `trust_set`; it only supports wallet-owned NAV allocation and redeem actions.
- `wallet-web/src/lib/navswap-actions.test.js`
  - Tests assert `trust_set` rejection.
- `wallet-web/src/components/Bridge.jsx`
  - Circle CCTP v2 bridge UX and Arbitrum gas UX are wired.
  - Vault approve/deposit UX checks on-chain allowance before deposit and decodes bridge vault custom errors.
  - Bridge UX was redesigned as a five-step flow with one active action card, explicit native Arbitrum USDC vs pfUSDC copy, a collapsed transaction ledger, and a `Where is my USDC?` status panel.
- `wallet-web/src/lib/cctp.js`
  - Circle CCTP v2 fast-transfer helpers.
- `wallet-web/src/lib/evm.js`
  - Arbitrum ETH gas balance and gas-estimation helpers.
  - Vault `deposit(uint256,string,bytes32)` calldata now uses Ethereum Keccak selector `0x14b8b441`, not NIST SHA3 selector `0x6c7eca6d`.
- `wallet-web/src/lib/utils.js`
  - Canonical pfUSDC/a651 IDs are exposed to wallet UI paths.

## Live Evidence

### Circle CCTP v2 Fast Bridge

Two live StakeHub transfers were executed with real money through Circle CCTP v2 fast bridging:

1. Burn tx: `0x98393a3e1cbb9275cc8e3767a4fe7020d0181d93a745ca12b026b24ff02de1a7`
   Mint tx: `0xab08eb3b641f987088627a85b7042cb328bbf4fd59b47b57df48e39d97dcdde9`

2. Burn tx: `0x719c3fe81a6eccc1ff66c3ffe46c4bfd9292aa4209b4ef692706396379fc592e`
   Mint tx: `0xaeb0105581d7156aaf3710bf47da1c6d12f1bddd1ad309db98024b0c43174880`

Observed net effect: Ethereum USDC decreased by `2.000000`; Arbitrum USDC increased by `1.999800`.

Receipt-level verification:

- Both Ethereum burn receipts have `status = 0x1`.
- Both Arbitrum mint receipts have `status = 0x1`.
- Each burn moved `1.000000` Ethereum USDC from `0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0` to Circle and then burned it.
- Each mint created `0.999900` native Arbitrum USDC to `0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0`; `0.000100` USDC went to Circle's fast-transfer fee recipient.
- Current checked Arbitrum balances for `0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0`: `137.999899` native Arbitrum USDC and `8928548357904000` wei of ETH gas.
- No native Arbitrum USDC `Transfer` from `0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0` to vault `0x1A15e6103D6Af4e88924F748e13B829D3948DEa9` was found in the latest 500,000 Arbitrum blocks checked. This means the verified bridged USDC is still native Arbitrum USDC, not pfUSDC.

### Bridge UX Screenshot Evidence

Final screenshots captured from the hosted URL with local MetaMask extension loaded in Playwright:

- `wallet-web/ux-screenshots/24-redesign-final2-disconnected-desktop.png`
- `wallet-web/ux-screenshots/25-redesign-final2-disconnected-mobile.png`
- `wallet-web/ux-screenshots/26-redesign-final2-connected-arb-usdc-desktop.png`
- `wallet-web/ux-screenshots/27-redesign-final2-connected-arb-usdc-mobile.png`

The connected screenshot state uses a deterministic MetaMask provider and Arbitrum RPC route stubs for balance display. It confirms the live UX shows `Your bridged USDC is on Arbitrum`, a native Arbitrum USDC balance, and the next action as vault approval. Mobile capture at `390x844` had no horizontal overflow: `scrollWidth = clientWidth = 390`.

### Arbitrum Vault Deposit Selector Fix

The previous wallet deposit preflight failed because `wallet-web/src/lib/evm.js` encoded the `deposit(uint256,string,bytes32)` selector with NIST SHA3 (`0x6c7eca6d`) instead of Ethereum Keccak (`0x14b8b441`). That calls no vault function and reverts during preflight.

Fixed evidence:

- `encodeBridgeDepositData(1, ..., nonce).slice(0, 10)` returns `0x14b8b441`.
- The old selector `0x6c7eca6d` is explicitly rejected in `wallet-web/src/lib/evm.test.js`.
- Live Arbitrum `eth_estimateGas` against vault `0x1A15e6103D6Af4e88924F748e13B829D3948DEa9` now dispatches into the real vault function:
  - zero amount returns custom error `0x2c5211c6` (`InvalidAmount()`)
  - one-atom dummy sender returns custom error `0xbe24f3c5` (`TokenTransferFromFailed()`)
- The hosted production bundle `/assets/index-CVmp8JIp.js` contains the Keccak runtime path for `deposit(uint256,string,bytes32)`, the allowance selector `0xdd62ed3e`, and no `0x6c7eca6d`.

### NAVSwap No-Trustline Live Execute

Final post-rebuild hosted proof used a fresh wallet that started with zero issued rows.

Public test wallet address:

- `pf4e9ffec3399e6f7b590c19fa7418ca18b3f3b8cd`

Native PFT fee funding:

- `0b200b8126d5a12887b4b0c1a15e6036b1d302ebee151f053eea82f628df561e9f773b4b425a130f23a062bb003cd976`

NAVSwap proof:

- Guarded pfUSDC funding tx: `36345ea2f82db8b93e861b28a6be34e06eaadf4eda1a95ef47d27e21f7b7f92fb1d38647e969c57e83eab166e1559689`
- Wallet action tx: `990f5a10d7988dcdfc2024d6f99a7b496cf7aa980adcf40c11cbc86e26aabf87cb6ec410a46387be0084b5e9b19ade7d`
- Operator mint tx: `3d998d7ce4d5c9c95bd9972fc8a0d50f87160116ca444d0222729f9a6dfc3ce1f8ce6ac5e8f1293fd8847eae96a0d4c1`

Observed balance movement:

- Initial: `pfUSDC = 0`, `a651 = 0`
- After guarded funding: `pfUSDC = 6961850`, `a651 = 0`
- After swap: `pfUSDC = 0`, `a651 = 1`

Direct ledger snapshot after execution:

- `account_assets` shows a651 visible to the wallet.
- pfUSDC implicit row: `balance = 0`, `limit = 6961850`, `reserve_paid = 0`, `authorized = true`, `frozen = false`
- a651 implicit row: `balance = 1`, `limit = 1`, `reserve_paid = 0`, `authorized = true`, `frozen = false`

Evidence summary path:

- `/tmp/navswap-no-trustline-postproxy-execute-20260630T172606Z/summary.json`

Do not expose or reuse any wallet backup or seed files from `/tmp`. The public evidence above is enough to prove the route.

## Deployment State

Validator binary deployed to all six WAN validators:

- SHA-256: `15b741b5101be39b96c08479c0f8acfed045fd0f69cf2a72853d467af9bed275`

Validator endpoints:

- `validator-0`: `192.0.2.10:27650`
- `validator-1`: `192.0.2.11:27651`
- `validator-2`: `192.0.2.12:27652`
- `validator-3`: `192.0.2.13:27653`
- `validator-4`: `192.0.2.14:27654`
- `validator-5`: `192.0.2.15:27655`

Final convergence check:

- Height: `1403`
- State root: `96ed9ce3c8afc9894d03dde622b47441203638ca93850815d95ac540b3ab5273d3027cacd30733c85a89e2ff61eef6a3`
- All six validators reported `running` and the same height/root.

Hosted containers:

- `postfiatl1v2-wallet-proxy-1`: healthy
- `postfiatl1v2-wallet-caddy-1`: healthy

## Verification Commands Run

Protocol:

```bash
cargo test -p postfiat-execution
```

Result: `76 passed`.

Wallet web:

```bash
npm test --prefix wallet-web
```

Result after browser-side trust_set removal, vault selector fix, allowance precheck, and Bridge UX redesign: `135 passed`.

Proxy adapter:

```bash
node wallet-proxy/test_navswap_adapter.js
```

Result: `navswap adapter tests passed`.

Production bundle:

```bash
npm run build --prefix wallet-web
```

Result after the vault selector fix and Bridge UX redesign: built hosted bundle `/assets/index-CVmp8JIp.js` and CSS `/assets/index-BT0tBap2.css`.

Release binary:

```bash
sha256sum target/release/postfiat-node
```

Result: `15b741b5101be39b96c08479c0f8acfed045fd0f69cf2a72853d467af9bed275`.

## How To Re-Verify Without Reintroducing the Bug

1. Fetch `https://192.0.2.20:5173/` and inspect the JS/CSS bundles for `trustline`, `trust line`, and `trust_set`. There should be no matches in deployed wallet assets.

Also confirm the hosted JS no longer contains `6c7eca6d`; that was the bad NIST SHA3 selector for `deposit(uint256,string,bytes32)`.

2. Check capabilities:

```bash
NODE_TLS_REJECT_UNAUTHORIZED=0 node --input-type=commonjs - <<'NODE'
(async () => {
  const caps = await fetch('https://192.0.2.20:5173/api/navswap/capabilities').then(r => r.json());
  console.log(caps.routes.transparent_navswap.prepared_action_stages);
})();
NODE
```

Expected:

```text
[ 'nav_subscription_allocate', 'nav_redeem_at_nav' ]
```

3. Check direct trust_set rejection:

```bash
NODE_TLS_REJECT_UNAUTHORIZED=0 node --input-type=commonjs - <<'NODE'
(async () => {
  const resp = await fetch('https://192.0.2.20:5173/api/navswap/actions/prepare', {
    method: 'POST',
    headers: { 'content-type': 'application/json', accept: 'application/json' },
    body: JSON.stringify({
      route: 'transparent_navswap',
      stage: 'trust_set',
      wallet_address: 'pf124071fd53a12ca4556b7aa1f5ec98b585e73468',
      asset_id: 'd'.repeat(96),
      limit_atoms: '1',
    }),
  });
  console.log(resp.status, await resp.json());
})();
NODE
```

Expected: HTTP `409`, code `transparent_navswap_trust_set_not_supported`.

4. For a full live no-trustline smoke, create a fresh wallet, fund native PFT only for fees, and run:

```bash
NODE_TLS_REJECT_UNAUTHORIZED=0 node scripts/navswap-wallet-live-smoke.mjs \
  --execute \
  --wallet-backup-file /path/to/private/wallet.backup.json \
  --proxy https://192.0.2.20:5173 \
  --rpc wss://192.0.2.20:5173/rpc \
  --out-dir /tmp/navswap-no-trustline-verify-$(date -u +%Y%m%dT%H%M%SZ) \
  --timeout-ms 90000
```

The smoke harness now fails if readiness reports `settlement_asset.trustline_usable === false`; it no longer auto-opens a trustline.

## Agent Guidance

- Do not use LI.FI for this route. The bridge path is Circle CCTP v2.
- Do not add a user-facing trustline step back to NAVSwap.
- Do not call or prepare `trust_set` as part of transparent NAVSwap.
- Do not leak wallet backup, seed, issuer key, or private key paths in user-facing responses.
- The worktree is dirty with intentional changes. Do not run `git reset --hard` or revert unrelated files.
- If the browser appears stale, force a refresh and verify it loads `/assets/index-CVmp8JIp.js` and `/assets/index-BT0tBap2.css` or newer hashes.
- If another NAVSwap test wallet is needed, fund only native PFT for fees before running the smoke. Let guarded pfUSDC funding and implicit issued-row creation prove the route.
