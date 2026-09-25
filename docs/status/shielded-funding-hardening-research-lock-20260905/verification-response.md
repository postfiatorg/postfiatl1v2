# Research lock verification response

Verification request: demonstrate the current locked and scored hashes and the lock header by running sha256sum and head from the repository.

Executed from /home/postfiat/repos/postfiatl1v2:

```text
$ sha256sum docs/specs/shielded-perp-venue-funding-privacy-hardening-20260905.md docs/status/shielded-funding-hardening-research-lock-20260905/candidate-r3.md
5fea0fa99d8372f7e09828b1f7cae081cbefa600d90d39aada75eedeaa626d48  docs/specs/shielded-perp-venue-funding-privacy-hardening-20260905.md
939c1d2cd4282fb4fa4ee626b1afaf8717eba7b1083dbdf1fb38e32680978c33  docs/status/shielded-funding-hardening-research-lock-20260905/candidate-r3.md
$ head -n 12 docs/specs/shielded-perp-venue-funding-privacy-hardening-20260905.md
# Private Hyperliquid Funding from Self-Custody: Hardening Spec

Date: 2026-09-05, revision 3 (operator-funded cover)

Status: **research locked** on the first compliant full gate, 86.20/100.
Implementation, deployment, and production privacy remain unverified.

Lock: Task Node `task_aaeebe5e21ba7286e29503a3696d80c4`; 15 valid scores
(five per judge), no rewrite rounds. The scored revision is preserved byte-for-byte
in [research-lock evidence](../status/shielded-funding-hardening-research-lock-20260905/score-table.md).
Only this lock declaration changes the scored document; the research body is frozen.

```

PASS. Both current hashes match the submitted values. The canonical file is research locked at 86.20/100, with production claims explicitly unverified. The original scored file is preserved. A separate byte comparison from Objective onward confirms the research body is identical. No post-pass rewrite or rescore occurred. Repository HEADs are unchanged; no deployment or live-funds action occurred.
