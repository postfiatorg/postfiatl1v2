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

**Who may cancel.** The destination owner, or the verifier's pause authority, signing, at any time. Nobody else may cancel, and there is no time limit.
- **Owner:** the signer's key, hashed with the light's Solana genesis, must equal the proven destination.
- **Pause authority:** the signer is the light's `pause_authority`. It cancels burns whose owner cannot sign, after people have checked the burn (`ops/sol-live/RUNBOOK-LIVE.md`, "Cancel a burn with the pause key").
- **Anyone else:** refused with `CancelNotAllowed` (error 9).
- **Why this rule:**
  - An earlier draft let anyone cancel once the light stood 256 Post Fiat blocks past the burn. Those blocks, and the proof that moves the light, are cheap for a stranger. During a prover outage, one stranger could then have cancelled every pending withdrawal.
  - A cancel moves no funds, and the pause key can already stop every payout. So the pause authority gains no new power: a stolen pause key can grief, but cannot steal.
  - A later program version could add a firmer permissionless rule, for example a two-step cancel timed in Solana slots. A payout landing within that window would still win.

**Pause.** A paused verifier refuses cancels as it refuses payouts.

**The race.** A payout and a cancel create the same marker account, and Solana orders the two transactions:
- whichever lands first creates the account;
- the other fails on the existing account (`Replay`, or the system program refuses the create);
- the failing transaction moves no funds.

So the user is paid (byte 1) or refundable (byte 2), never both, and never neither. `upgrade_rehearsal.py` exercises both orders.

**Before the second release height.** The cancel exists from the Solana upgrade on, but the refund decision only from the second release's height. A burn cancelled in between can be neither paid nor refunded until that height. Only the owner or the pause key can cause this, so the gap is closed by procedure:
- the pause key signs no cancel before the height is active;
- no user-facing cancel exists before then;
- the worker tells such a user that the refund comes once the second release is active.

**Limits.**
- `exit_govern.py refund` finds the cancel only as a top-level instruction. A cancel made through another program, for example by a program-owned destination, is not found. Such owners use the pause-authority path instead.
- A burner who has removed the trust line for the burned series cannot be credited until they re-create it. The refund refuses until then and can be repeated afterwards.

## Post Fiat refund

The refund sits behind the second release's own height.

**Kind and signers.** A governance decision `solana_exit_refund:<redemption id>:<finalized slot>:<cancel signature>` needs the same quorum as a paid decision under the second release: five of six. Each signer's own host reads the marker of that burn at a finalized slot, through that host's own Solana endpoint, checks the byte `2` and the cancel transaction, and only then signs (`ops/sol-live/exit_review.py`, runbook "Independent settlement reviews" and "Refund a cancelled burn").

**Effect.**
- The redemption must be pending and unsettled, in a covered bucket.
- The burner is credited back the burned amount, in the same series, from the same bucket.
- The bucket's outstanding amount rises by that amount, its redemption queue falls by it, and the series supply rises by it. Counted reserves do not change, because the USDC never left the vault.
- The redemption becomes terminal as refunded. A second refund, or a paid decision after it, is refused.

## Service and user

**The worker.** It reads the marker before it proves and again before it sends. Byte `1` means paid, byte `2` means cancelled. A cancelled burn ends the job as failed with the fixed reason `cancelled: refund on Post Fiat` (before the second release: `… once the second release is active`), and it is not retried. The custody check counts only paid markers as USDC that has left the vault.

**In-flight payout.** If a payout is in flight when the cancel arrives, Solana orders the two transactions. Whichever lands first creates the marker, and the other fails on the existing account. A payout that loses ends the job as cancelled; a cancel that loses stays without effect, and the burn is paid.

**The user.**
- **While waiting:** the withdrawal shows the named reason.
- **Once five validators have signed the refund:** the pfUSDC is back in the same series.
- **Before the second release height:** a cancel can already be written on Solana, and its refund waits for that height. The relay offers cancel only from that height.
