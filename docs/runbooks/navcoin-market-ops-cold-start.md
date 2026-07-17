# NAVCoin Market Operations Cold-Start Playbook

Status: controlled-launch playbook
Audience: launch captain, Ethereum operator, PFTL operator, reviewer

This playbook starts a NAVCoin market-operations venue conservatively. It does
not create a redemption path. PFTL publishes NAV and bounded market-operation
caps; Ethereum contracts enforce accepted envelopes and expiry.

The conservative launch parameters are pinned in
`docs/examples/navcoin-market-ops-initial-parameters.json`.

## Preconditions

- Reserve and supply packet production is live and replayable.
- The policy program id, policy hash, parameter hash, pool config hash, and
  hook code hash are known before deployment.
- Ethereum deployer keys are funded only for deployment and operation.
- PFTL replay operators can export and verify replay bundles.
- Public status publication is ready before any nonzero cap is accepted.

## Initial Parameters

Use the checked JSON fixture as the launch source of truth:

```bash
scripts/navcoin-market-ops-cold-start-check \
  --parameters docs/examples/navcoin-market-ops-initial-parameters.json \
  --runbook docs/runbooks/navcoin-market-ops-cold-start.md
```

The initial policy is deliberately narrow:

- observe-only mode at launch;
- 14 days of venue observations before automatic reserve deployment;
- 30 days of venue history before any premium mint cap can be nonzero;
- zero premium mint at launch;
- one eligible route;
- committed NAVGuardHook and pool configuration;
- 1 percent slippage limit for replay quotes;
- 24 hour bridge challenge delay;
- 1 hour envelope execution window;
- full funding required before nonzero reserve-deploy caps;
- initial per-epoch reserve deployment capped at 10 percent of the funded
  alignment reserve and additionally capped by deterministic policy output.

## Deployment Sequence

All command values below are placeholders until the launch packet binds real
addresses and hashes.

1. Deploy the a651 token or register the existing token address.

   ```bash
   export A651_TOKEN=<token-address>
   export USDC_TOKEN=<usdc-address>
   ```

2. Deploy `PolicyRegistry`.

   ```bash
   forge create \
     --rpc-url "$ETH_RPC_URL" \
     --private-key "$DEPLOYER_PRIVATE_KEY" \
     crates/ethereum-contracts/src/PolicyRegistry.sol:PolicyRegistry \
     --constructor-args "$POLICY_OWNER"
   ```

3. Deploy `MarketOpsVault`.

   ```bash
   forge create \
     --rpc-url "$ETH_RPC_URL" \
     --private-key "$DEPLOYER_PRIVATE_KEY" \
     crates/ethereum-contracts/src/MarketOpsVault.sol:MarketOpsVault \
     --constructor-args "$USDC_TOKEN" "$A651_TOKEN" "$VAULT_OWNER" 500 true
   ```

4. Deploy `MintController`.

   ```bash
   forge create \
     --rpc-url "$ETH_RPC_URL" \
     --private-key "$DEPLOYER_PRIVATE_KEY" \
     crates/ethereum-contracts/src/MintController.sol:MintController \
     --constructor-args "$A651_TOKEN" "$MINT_OWNER" "$UNIT_SCALE"
   ```

5. Register the policy binding.

   The registration must bind:

   - `program_id`;
   - `policy_hash`;
   - `parameter_hash`;
   - `venue_id`;
   - `pool_config_hash`;
   - `hook_code_hash`;
   - activation epoch;
   - optional deactivation epoch.

6. Deploy `PFTLBridgeAdapter`.

   ```bash
   forge create \
     --rpc-url "$ETH_RPC_URL" \
     --private-key "$DEPLOYER_PRIVATE_KEY" \
     crates/ethereum-contracts/src/PFTLBridgeAdapter.sol:PFTLBridgeAdapter \
     --constructor-args \
       "$POLICY_REGISTRY" \
       "$BRIDGE_OWNER" \
       "$CHAIN_ID" \
       "$MARKET_OPS_VAULT" \
       "$MINT_CONTROLLER" \
       86400 \
       3600 \
       1800
   ```

7. Wire the adapter.

   ```bash
   cast send "$MARKET_OPS_VAULT" \
     "setBridgeAdapter(address)" "$PFTL_BRIDGE_ADAPTER" \
     --rpc-url "$ETH_RPC_URL" --private-key "$VAULT_OWNER_PRIVATE_KEY"

   cast send "$MINT_CONTROLLER" \
     "setBridgeAdapter(address)" "$PFTL_BRIDGE_ADAPTER" \
     --rpc-url "$ETH_RPC_URL" --private-key "$MINT_OWNER_PRIVATE_KEY"
   ```

8. Deploy `NAVGuardHook` and create the Uniswap v4 pool with the committed hook
   and pool configuration.

   The launch packet must record the hook address, hook code hash, pool id,
   PoolManager address, fee mode, route semantics, and the exact
   `quote_cost_to_reach_price` implementation.

9. Configure the vault venue.

   ```bash
   cast send "$MARKET_OPS_VAULT" \
     "setVenue(bytes32,address)" "$VENUE_ID" "$APPROVED_VENUE_ROUTER" \
     --rpc-url "$ETH_RPC_URL" --private-key "$VAULT_OWNER_PRIVATE_KEY"
   ```

10. Seed liquidity with disclosed operator capital.

    Record the source account, amount, timestamp, pool id, and resulting pool
    state in the launch packet. Do not enable automatic caps in this step.

11. Fund the venue-specific alignment reserve.

    ```bash
    cast send "$USDC_TOKEN" \
      "approve(address,uint256)" "$MARKET_OPS_VAULT" "$INITIAL_ALIGNMENT_RESERVE_USDC" \
      --rpc-url "$ETH_RPC_URL" --private-key "$FUNDER_PRIVATE_KEY"

    cast send "$MARKET_OPS_VAULT" \
      "fundVenueReserve(bytes32,uint256)" "$VENUE_ID" "$INITIAL_ALIGNMENT_RESERVE_USDC" \
      --rpc-url "$ETH_RPC_URL" --private-key "$FUNDER_PRIVATE_KEY"
    ```

12. Accumulate NAVGuardHook observations for at least the minimum observation
    window. Keep the policy in observe-only mode.

13. Run PFTL calibration in observe-only mode.

    ```bash
    postfiat-node replay-envelope --bundle "$OBSERVE_ONLY_BUNDLE_DIR"
    ```

14. Publish replay bundles and expected envelope hashes.

    The bundle must bind reserve packet hash, supply packet hash, EVM evidence
    root, policy identity, parameter hash, previous market-state hash, and
    expected envelope hash.

15. Enable small reserve-deployment caps only after replay passes and public
    status shows fresh reserve and supply packets, adequate funding, accepted
    policy hash, envelope epoch, and packet expiry.

16. Enable premium mint caps only after the premium-history window has elapsed,
    adversarial tests are green, and post-mint backing checks remain green.

## Stop Conditions

Stop and leave caps at zero if any of these occur:

- policy registration does not match the envelope bindings;
- hook code hash or pool config hash differs from the launch packet;
- reserve or supply packets are stale;
- EVM evidence root does not verify;
- route or pool state cannot be replayed exactly;
- bridge packet is challenged;
- PFTL finality equivocates or halts;
- alignment reserve is underfunded;
- public status publication is unavailable.

## Launch Record

The launch captain records:

- deployed contract addresses;
- token address and decimals;
- policy registration fields;
- pool id and PoolManager;
- hook address and hook code hash;
- pool config hash;
- reserve and supply packet hashes;
- EVM evidence root;
- replay bundle path and expected envelope hash;
- initial funding receipt;
- first accepted envelope id;
- public status URL or artifact.
