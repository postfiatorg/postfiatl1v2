# Storage-scaling development evidence

Status: **PUBLIC TESTNET BLOCKED**

This directory begins the implementation evidence for the
[Storage Scaling and Bounded Finality Research Specification](../../docs/architecture/storage-scaling-research-spec.md).
The current packet is deliberately narrow: it binds development measurements
showing that the new JSONL append and ordered-history proposal/index work do not
read or verify a chain-length-dependent prefix through synthetic height 5,000.

The controlling counters are the useful result. Normal JSONL append verified
zero accepted-prefix records at every sampled height. Ordered-history proposal
and append work read a fixed-size authenticated bitmap, touched at most the
fixed probe limit, and materialized no historical batch list after activation.
The offline rebuild remains linear by design because it authenticates and
derives the index from canonical history.

The elapsed values were produced by unoptimized unit-test binaries on one ext4
host. They are not finality measurements or an SLA. In particular, the current
fixed bitmap design costs about 0.33 seconds for a proposal lookup and about
0.64 seconds for an index append in this debug harness. Candidate optimization
and the complete paired six-validator benchmark remain open.

Evidence is in [e2-bounded-work.json](e2-bounded-work.json), bound to source
commit `dfd0b9f11108b0b773d1e02bebae71685864228e`.

The storage milestone has not passed. Archive/live-history replay, the complete
tamper corpus, paired height-50/height-5,000 finality, clone migration and
rollback, CLI packet verification, and the read-only browser interface are
still required before public-testnet readiness can be reconsidered.
