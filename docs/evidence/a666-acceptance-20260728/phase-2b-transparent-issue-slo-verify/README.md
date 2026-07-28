# A666 Phase 2b Transparent Issue Verification

**Date:** 2026-07-28

**Gate:** A2 — transparent issue verified

**Verdict:** PASS

The fresh live mainnet run completed without human action between the USDC
deposit and terminal wA666 readback:

```text
1.005000 Ethereum USDC
  -> 1.005000 PFTL pfUSDC
  -> 1.000000 newly issued A666
  -> 1.000000 Ethereum wA666
```

Deposit inclusion to spendable wA666 was `1,464 seconds` (`24m24s`), passing
the `1,500-second` SLO by 36 seconds.

## Identifiers

- Ethereum deposit:
  `0xfda3bc55156774b9c35b16d03eea1832ca5f269fed3670b2956a3149c2852892`
- PFTL deposit ID:
  `bc67d1fe2cc5f12179214dd8582ef23f6a6d55dc15f53794566ac6156105998f`
- PFTL export packet:
  `2ab16576034b1f0b78d1e9ba898f073399268e3a2a76249121a547a6cc1e1a3f6bb223b8152181b218354898d07bfead`
- Ethereum packet digest:
  `0xf45f930b961c38f688156f86c18a6f935b69e12f240a872b47854566560a3570`
- Proof acceptance:
  `0xae36129808964efe433cdda443b8f6f6bf438bd333e578c40036ce1892aa884b`
- wA666 mint:
  `0x8fdd2d5f160e424733b3dcb3c1798f52518ce72d0179ad8a6e32f3a45649a421`
- PFTL height:
  `368 -> 373`

## Acceptance results

- The campaign manifest was frozen and pushed before value moved.
- All six validators began at the same height, tip, state root, binary hash,
  topology hash, and empty mempool.
- Ethereum finality covered the exact deposit before PFTL credit.
- The ingress proof used the frozen SP1 vkey and reported the CUDA backend.
- Joe's new `1.000000 A666` PFTL balance was observed after subscription and
  before export.
- The export proof used the deployed receipt vkey and exact finalized packet.
- Joe's wA666 balance and total supply each increased by exactly `1.000000`.
- The A651 migration reserve did not change.
- The live Uniswap pool retained exactly the same liquidity; primary issuance
  did not trade through it.
- An ERC-20 transfer simulation for the received wA666 returned `true`.
- All six validators ended at height 373 with one identical tip and state
  root and empty mempools.
- Exact PFTL export replay rejected as `bad_sequence` without mutation.
- Exact Ethereum proof replay reverted as `ProofAlreadyConsumed`.
- Exact Ethereum mint replay reverted as `PacketReplay`.

Machine-readable acceptance is in `acceptance-summary.json`; exact accounting
is in `conservation.json`; block-timestamp measurement is in `timing.json`.

## Intervention record

There were zero human actions between deposit broadcast and terminal
readback. A pre-deposit evidence loop initially recorded only validator 0
because SSH consumed the piped host list. It was corrected before funds moved,
then all six validator records were required and verified. This did not alter
chain state, the frozen transaction intent, or the live workflow.

## Privacy statement

This was intentionally transparent. Ethereum deposit and mint, PFTL balances
and operations, supply changes, destination, amount, and timing are public.
No privacy claim is made for this phase.
