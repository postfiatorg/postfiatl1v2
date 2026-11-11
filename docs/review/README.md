# Reviews

> **Internal engineering record.** This directory holds internal review
> packets, QA campaign logs and decision proposals. They are dated, written by
> the project, and are not external audits. For the security position, read
> [Security](../security/index.md).

## Start with

| Page | What it is |
| --- | --- |
| [QA defect inventory, 2026-09-10](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/review/defect-inventory-20260910.md) | Every finding from the internal QA campaigns: 142 rows (27 P1, 77 P2, 38 P3) with status |
| [NAVCoin specification platform review, 2026-10-01](navcoin-spec-platform-review-20261001.md) | Correctness review of the NAVCoin create-and-swap specification against the deployed validator code |
| [BMNRC protocol enforcement check, 2026-10-02](bmnrc-protocol-enforcement-check-20261002.md) | What the validator code enforces for BMNRC, and what the chain state shows |
| [Z3 one-cycle review packet, 2026-11-11](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/review/z3-cycle1-review-packet-20261111.md) | What one Arc-to-A666 round trip does, its amounts, stop conditions, the G2 result and the inputs still waiting |

## Everything else

The other files are per-area review reports (consensus, storage, mempool,
wallet, bridge, NAV reserve proofs, Task Node UNL and others), QA campaign
briefs and logs (`qa-campaign-*`), and decision proposals. They and the defect
inventory are not published on the docs site, so those links go to the
[repository](https://github.com/postfiatorg/postfiatl1v2/tree/main/docs/review).
Other records refer to these file paths, so do not move or rename them.
