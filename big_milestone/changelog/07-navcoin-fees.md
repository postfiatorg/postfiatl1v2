# Changelog — Pillar 7 — Transparent Fees for NAVCoins

How each reviewer problem was handled, round by round. The spec itself is [here](../pillars/07-navcoin-fees.md); the reviews are under [feedback](../feedback/README.md).

## Round 1 review → revision 1

Reviews: [round 1](../feedback/round-1/07-navcoin-fees.md).

| Item | Change |
|---|---|
| Gemini P1 | Options A and B specified exactly; open question 1 |
| Gemini P2 | Fee math: units, order, per-unit HWM, `F × S / (V − F)` mint, rounding, initial HWM; R12 |
| Gemini P3 | Δh from heights, capped; notice in blocks; acceptance 6; open question 3 |
| Gemini P4 | R9 unclaimed balance and claim; edge cases never fail; R10. In code, the mint cap is the packet's supply plus `max_supply` |
| Gemini P5 | R3 signer; R4 classification, notice reduction counts as an increase, one pending slot; open question 10 |
| Gemini P6 | R5 mapping and migration; R11. Partly disputed: `NAVGuardHook` records observations and does not price at NAV |
| Gemini P7 | `fix_fee_cap_bps` (R1, R6); R15 includes schedule transactions |
| Gemini P8 | "Deduct from reserves" removed, with the reason |
| Gemini P9 | R17–R19; fee column in the five-parts table; open questions 4 and 13 |
| Gemini P10 | Pillar 2 optional via `disclosure_ref` (its R1 already lists `pf.navcoin.fee.*`) |
| Gemini P11 | Ten acceptance criteria covering each failure mode |
| Opus P1 | `V`, `N`, `H` defined; performance fee after management |
| Opus P2 | Mint sizing fixed; example checked to the atom |
| Opus P3 | Δh, cap, catch-up and deadman; open question 3 |
| Opus P4 | Open question 4; R14 shows the proof split; R8 |
| Opus P5 | Option B for late entrants; per-holder marks ruled out; open question 5 |
| Opus P6 | R11: routes read the post-fee NAV; fee units export normally |
| Opus P7 | R5 migration; acceptance 8 |
| Opus P8 | R4 mixed and non-rate changes; open question 7 |
| Opus P9 | Spreads under notice (R5); open question 6 |
| Opus P10 | Acceptance 2 uses a drawdown path and works for both options |
| Opus P11 | R16 and "Why fees are paid in units" |
| Opus P12 | Fix fee cap; open question 8 |
| Opus P13 | R12 schema, R15 surfaces and RPC |
| Opus P14 | Open question 11 |
| Opus P15 | Checked. Fixed route op names (`PftlUniswap*`), the spread destination, the NAV op execution file, and "operator-only" as a release rule |
| Pillar 8 cross-check | R9: rules for the recipient's credential and line on gated NAVCoins; fee claims are not FastPay. R6/R15: the fix is a private swap, so the fix fee shows as a rate. Open question 6: de-authorised holders cannot exit |
