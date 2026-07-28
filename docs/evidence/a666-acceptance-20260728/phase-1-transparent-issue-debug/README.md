# Phase 1 — Transparent Primary-Issue Debug

**Functional verdict:** PASS

**25-minute SLO verdict:** FAIL (`2,304 seconds`, or `38.4 minutes`)

This was the intentionally diagnostic transparent run. Joe deposited exactly
`1.005000 USDC`; PFTL finalized exactly `1.005000 pfUSDC`; the primary market
spent that settlement to create exactly `1.000000 A666`; and the finalized
export minted exactly `1.000000 wA666` to Joe on Ethereum.

## Finalized transitions

| Transition | Result |
|---|---|
| USDC approval | `0x3c699a71fe68de96953d4ba3961f6bfd0b5eddd2b045096691729971460fc612` |
| USDC deposit | block `25,629,883`, tx `0xadecf2fe0b96b7aef2eaaa62ebeac33f16201f8014c784603486a46fe1a0cbb1` |
| pfUSDC propose / claim | PFTL heights `359` / `360` |
| A666 reserve / subscribe / export | PFTL heights `361` / `362` / `363` |
| Export-proof acceptance | block `25,630,068`, tx `0xe565814a40890ebc968188fae1bdba51e67989b451f635d195011068ed51ba8d` |
| One-time wA666 mint | block `25,630,075`, tx `0x3df713f469d62d88c5c6c3f0b7cd0af07b9b6a96d1fb5664514eb048a3194fbf` |

The PFTL export packet is
`72b2ccf189370d1a6012fd5a3fce3835612289a71f34b9807d3eea0655d0811b207e9b548c67a02b8178bd1581c1fd8d`.
Its Ethereum packet digest is
`0x8dbd308f439e3bed08e72e858c6d94e1f6b87e975d95aa63b2cb265383e113ef`.

## Exact conservation readback

- Joe pfUSDC after subscription: `0.800000`, equal to his pre-run balance.
- Joe A666 after subscription: `1.000000`.
- Joe A666 after export: `0`.
- Joe wA666: `100.000000` → `101.000000`.
- Authorized / wrapped A666 supply: `31,486.197455` → `31,487.197455`.
- A651 migration reserve: unchanged at `27,306.423797`.
- The mint packet is consumed exactly once.

## Defects found and fixed

1. The CUDA-capable SP1 binaries silently selected the CPU backend when
   `SP1_PROVER=cuda` was omitted. Both live proof commands were corrected, and
   both prover CLIs now support a fail-closed `--require-prover cuda` guard.
2. The Ethereum executor was initially given the pre-export packet template.
   Its receipt hash/root are intentionally zero before finality. Proof
   acceptance succeeded, but gas estimation rejected the invalid mint before
   broadcast. The executor now requires the finalized receipt witness for
   execution, derives the resolved packet from it, checks its internal
   consistency, and verifies that proof acceptance authorizes that exact
   packet before minting.
3. The mint executor previously had authority to unpause the controller. It
   now fails closed if the governed pause is set and never changes pause state.

The clean transparent verification run must exercise these guards and meet the
25-minute deposit-confirmation-to-wA666-mint SLO.
