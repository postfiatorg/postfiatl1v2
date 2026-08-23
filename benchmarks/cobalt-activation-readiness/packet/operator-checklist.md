# Cobalt Controlled-Testnet Cutover Checklist

Decision: **GO for a later, separately authorized validator-trust cutover.**
This packet does not authorize or perform activation. Foundation remains active and Consensus v2 remains block finality.

Before the separate cutover task:

- [ ] Refresh all six validator and shadow receipts.
- [ ] Confirm one registry root, one trust-graph root, contiguous signed history, and healthy Consensus v2 finality.
- [ ] Confirm any five valid validators can ratify and four cannot.
- [ ] Select a future activation height and collect distinct current-registry ML-DSA-65 approvals.
- [ ] Prepare and verify the forward Foundation rollback transition.
- [ ] Obtain explicit user authorization and govern the live cutover with a new Task Node task.

During cutover, stop on root disagreement, history gaps, validator churn, finality regression, or resource alarms.
