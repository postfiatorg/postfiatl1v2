# Phase 9 Transparent Issue Recovery and Completion

Status: `PASS` at PFTL height `416`

The frozen recovery point below was resumed without creating a second
deposit. The existing `1.005000 USDC` deposit produced `1.000000 A666`,
exported it to Ethereum, and increased Joe's wA666 balance from `103000000`
to `104000000` atoms. Destination consumption then moved the new unit from
outstanding bridge claims to Ethereum spendable supply while preserving the
route invariant.

The functional result is recorded in `summary.json`. The deposit-to-mint
elapsed time was `6660` seconds because this was a recovery of a deliberately
paused run; `timing.json` records `slo_pass:false` and does not represent this
run as satisfying the 25-minute fresh-run SLO.

The runner initially stopped after the successful destination-consume round
because its evidence assertion incorrectly expected outstanding bridge claims
to remain constant. The correct transition is:

```text
outstanding_bridge_claims -= minted_amount
ethereum_spendable_supply += minted_amount
authorized_valid_supply remains constant
```

This directory contains the frozen recovery point for workflow
`a666-p9-20260728`.

The `1.005000 USDC` deposit is canonical on Ethereum mainnet. It has not been
relayed, claimed, reserved, subscribed, or exported on PFTL. All six PFTL
validators remain converged at the pre-deposit baseline, height `410`, with
empty mempools.

Do not create another deposit against this intent.

## Exact next operation

Continue the existing ingress capture on the A100 host from:

```text
/workspace/a666-acceptance/live/a666-p9-20260728/ingress/deployment.json
```

That file is byte-for-byte identical to local
`ingress/capture-deployment.json`, SHA-256
`f5558e26b84ecc15c9d91aaf4b24cd50f299d723255c856ed67e542c64a76ff0`.

The GPU directory contains no witness and no proof. Validator 2 contains no
`/var/lib/postfiat/validator-2/a666-p9-20260728` directory. Therefore the next
operation is **Ethereum witness capture**, followed by ingress proof
generation. It is not a deposit retry and it is not a PFTL relay retry.

After the proof exists, the pinned issue inputs are:

| Input | Value |
|---|---|
| Start PFTL height | `410` |
| Prior verifier height | `403` |
| Prior checkpoint block ID | `bbc31d3ab0b3c3b7e459b0e7d598db0aca52189a545b98c10d60ad03bf177c91ce1a4ec5d25ed49c5d2e310421fd5f88` |
| wA666 recipient balance before | `103000000` atoms |
| wA666 total supply before | `31489197455` atoms |
| Deposit ID | `0xc1b73435029d42ebace223a0970d837736862da6569c9cf38cb7cef5c5ba5682` |
| Deposit transaction | `0x88f4c9ffc95568e1c44f422d8e7ba2162da70fb1fb753fd43b45458fd6cf4a48` |

Before any PFTL submission, repeat the read-only checks in
`recovery-checkpoint-20260728T203650Z.json`. Abort if the deposit record,
vault conservation, PFTL height/state root, Joe's pfUSDC balance, active
reservation count, export entitlement, or mempool state differs.

`artifact-sha256.txt` authenticates the lightweight recovery packet. It does
not cover the unrelated untracked deployment archives elsewhere in the
working tree.
