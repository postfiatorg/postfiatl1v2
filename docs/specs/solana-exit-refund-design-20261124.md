# Refund of an unpayable Solana exit burn

Status: design for the second Solana withdrawal release (PR 223). The Solana half ships in the same verifier upgrade as the route migration (`ops/sol-live/RUNBOOK-LIVE.md`, "Upgrade the initialised Solana verifier").

## Problem

A Solana exit is two steps. First, the holder burns pfUSDC on Post Fiat. Then the payout worker proves that burn to the verifier on Solana, and the verifier releases USDC from the vault. If the second step can never happen, the pfUSDC is gone and the USDC stays in the vault. Nothing returns it today.

A burn can be unpayable when:

- the destination owner's USDC token account is frozen, or was closed and cannot be re-created for that owner;
- the USDC mint's rules change (a freeze, a transfer hook or a new account requirement) so that the vault's transfer fails;
- the owner is not a wallet key (a program-derived or off-curve address). The second release refuses this before the burn, but a tool that skips the checks, or a first-release revision-8 burn made before the checks existed, can still carry one.

The second release refuses the known bad cases before the burn: an owner that is not a wallet key, an amount under the minimum, and a destination that the service's owner check refuses. The cases above appear only after the burn.

**Hard rule.** A burn is never both paid on Solana and refunded on Post Fiat.

## Options

**(a) Cancel on Solana, refund against the cancelled marker.** A payout already writes the burn's one-use marker account (seed `egress-burn` and the burn's hash) and refuses when that account exists. A new verifier instruction, `cancel`, writes the same account with a different state:

| Marker state | Data byte |
| --- | --- |
| none | (no account) |
| paid | `1` |
| cancelled | `2` |

Both instructions create the account and refuse if it exists. So "paid" and "cancelled" exclude each other on Solana by construction, whatever order the transactions land in. Post Fiat refunds only a burn whose marker reads `2` at a finalized Solana slot. Five of the six validators attest to that marker; this is the same governance shape that already settles a paid burn (`solana_exit_r8_paid:…`).

**(b) Refund decided on Post Fiat alone after a time limit.** Post Fiat cannot see Solana, and the verifier cannot see a refund. A payout already in flight, or the worker retrying a burn whose destination was fixed later, would pay on Solana after the refund. That breaks the hard rule. Stopping it would need the verifier to learn of the refund, which needs a proof of the refund decision on Solana and a marker anyway: that is (a) with an extra step. So (b) cannot keep the rule on its own.

**Chosen: (a).**

## The cancel

**Proof.** A cancel carries the same values and Groth16 proof as the payout of that burn. Without them, anyone could block any burn ID. The verifier checks it exactly as for a payout: chain, prior checkpoint, proof, then the route, vault alias and mint alias of the light. The values give the burn's own Post Fiat block height.

**Who may cancel.**
- **The destination owner, at any time.** The owner signs, and the owner's key must hash to the burn's proven destination. A holder whose token account is frozen or closed does not have to wait.
- **Anyone, once the light has moved on.** The verifier's light must stand at least `CANCEL_AFTER = 256` Post Fiat blocks past the burn's block. This covers owners that cannot sign.
- **Not the validators.** The verifier cannot check their ML-DSA signatures, and the two paths above cover every case.

**Time limit.** The limit is counted in Post Fiat blocks as the light account sees them: `light.height − burn height ≥ 256`. `light.height` is the latest Post Fiat block proven to the verifier, by a checkpoint or a payout. Post Fiat blocks are made on demand, so this is a count of later finalized blocks, not of minutes. The worker pays a payable burn within minutes, and it advances the light while doing so.

**Pause.** A paused verifier refuses cancels as it refuses payouts.

## How firm is the time limit

**An outsider can shorten it.** Post Fiat makes a block whenever transactions arrive. Anyone with an account and fees can cause the 256 blocks with plain transfers. At today's finality of about five seconds a block, that takes about 20 minutes and 256 fees.

**Who advances the light, and at what cost.** Blocks alone do not move the light. `light.height` rises only when someone proves a later finalized block to the verifier: a checkpoint (tag 8), a payout or a cancel. Each of these is permissionless. Each needs a Groth16 proof of public chain data, made with the SP1 prover in two to four minutes of GPU time. A checkpoint also pays the rent of a new checkpoint account, about 0.0009 SOL, plus fees.
- The worker advances the light itself when it falls more than 65 blocks behind a burn.
- A determined outsider with a prover can do it too.

**The race.** A payout and a cancel create the same marker account. Solana orders the two transactions:
- whichever lands first creates the account;
- the other fails on the existing account (`Replay`, or the system program refuses the create);
- the failing transaction moves no funds.

So the user is paid (byte 1) or refundable (byte 2), never both, and never neither. The worker reads a cancelled marker as final (`cancelled: refund on Post Fiat`), and the refund decision needs that marker. `upgrade_rehearsal.py` exercises both orders with real proofs. `anyone_may_cancel_only_after_the_time_limit_seen_by_the_light` and `the_time_limit_moves_only_with_the_light` show that the anyone path depends on the light alone.

**What it costs a user.** A stranger who cancels a payable burn cannot take funds. The holder gets the pfUSDC back after a refund decision instead of USDC on Solana: a delay, and a second burn if the holder still wants to exit. This can happen while a payout is legitimately pending, for example while the proving hosts are down for hours, if an outsider pays to make 256 blocks and to prove one checkpoint. That is griefing, not loss.

**Options for a firmer rule.** These are not implemented; each must ship in the same upgrade, or wait for a later one.
1. **Two-step cancel on Solana time.**
   - Anyone may *request* a cancel with the burn's proof. This writes a request account holding the Solana slot.
   - The cancel completes only after a fixed number of Solana slots, for example three days.
   - A payout in that window still wins, because it creates the marker first.
   - Solana slots advance whatever Post Fiat does, so outsiders cannot shorten the window.
   - Cost: one instruction, one account kind and a slot check (about 40 lines), and a rehearsal that can warp slots (`solana-program-test`).
   - This is the recommended rule for the single upgrade, if the owner accepts one more review round.
2. **A larger block count**, for example 4,096. This is simple, but it is still shortened by cheap blocks. It only raises the cost.
3. **Only the destination owner may cancel.** Burns whose owner cannot sign would then wait for a Post Fiat decision and a later upgrade.

The code keeps the 256-block rule this round.

## Post Fiat refund

The refund sits behind the second release's own height.

**Kind and signers.** A governance decision `solana_exit_refund:<redemption id>:<finalized slot>:<cancel signature>` needs the same quorum as a paid decision under the second release: five of six. Each signer reads the marker of that burn at a finalized slot, checks the byte `2` and the cancel transaction, and only then signs.

**Effect.**
- The redemption must be pending and unsettled, in a covered bucket.
- The burner is credited back the burned amount, in the same series, from the same bucket.
- The bucket's outstanding amount rises by that amount, its redemption queue falls by it, and the series supply rises by it. Counted reserves do not change, because the USDC never left the vault.
- The redemption becomes terminal as refunded. A second refund, or a paid decision after it, is refused.

## Service and user

**The worker.** It reads the marker before it proves and again before it sends. Byte `1` means paid, byte `2` means cancelled. A cancelled burn ends the job as failed with the fixed reason `cancelled: refund on Post Fiat`, and it is not retried. The custody check counts only paid markers as USDC that has left the vault.

**In-flight payout.** If a payout is in flight when the cancel arrives, Solana orders the two transactions. Whichever lands first creates the marker, and the other fails on the existing account. A payout that loses ends the job as cancelled; a cancel that loses stays without effect, and the burn is paid.

**The user.**
- **While waiting:** the withdrawal shows the named reason.
- **Once five validators have signed the refund:** the pfUSDC is back in the same series.
- **Before the second release height:** a cancel can already be written on Solana, and its refund waits for that height. The relay offers cancel only from that height.
