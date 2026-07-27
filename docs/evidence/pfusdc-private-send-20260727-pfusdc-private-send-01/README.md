# Live private pfUSDC send

Verdict: `PASS`

Run `20260727-pfusdc-private-send-01` moved 100,000 atoms (0.1 pfUSDC) from an
Asset-Orchard note controlled by wallet A to a private note recoverable and
spendable by distinct wallet B.

The live sequence finalized pfUSDC ingress at H334, a same-value a651 helper
ingress at H335, and the private send at H336. The existing fixed
two-input/two-output Asset-Orchard swap circuit carried the pfUSDC output to
wallet B and returned the conserved a651 helper value to a wallet-A-controlled
note. This is a narrow private-send construction, not the future general
single-asset transfer action.

Public evidence:

- `gate.json`: top-level acceptance result, heights, balance deltas, replay
  result, and embedded verification.
- `private-send-verification.json`: recipient recovery and note status, input
  nullifier status, conservation, and privacy checks.
- `fleet-attestation.json`: six-validator H336 convergence and round timings.
- `public-private-send-action.json`: exact finalized public action.
- `public-private-send-batch.json`: exact finalized public batch.
- `replay-rejection.txt`: concise exact-batch replay rejection.

The public action SHA-256 is
`144ca5bb02ea75061ce24fdaa56465c593b5189872d04f7d29253e9764bfbdae`.
The public batch SHA-256 is
`205174396b858a60ed44366ed9df6ed3a342740a22b9ef1c63e9e43467cb2de3`.

No wallet seed, private note opening, signing key, or private holder key is in
this evidence directory. Private wallet material remains mode-0700 on the
source validator.

Privacy boundary: raw asset IDs, amounts, sender, recipient, memo, and wallet
seed are absent from the public action. Stable asset tags and the 1:1 pricing
claim are public. The two ingress operations are public boundaries.
