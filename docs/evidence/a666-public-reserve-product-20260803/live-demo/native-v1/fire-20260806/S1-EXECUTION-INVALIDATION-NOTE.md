# S1 execution invalidation

**Status: SUPERSEDED FOR EXECUTION.**

The original S1 binding (`binding-S1.json`, SHA-256 `cc1bd291543e45e59fa2ff89df7e5c041c8ed101d6f93fb7e0eac57dd134bf9c`) remains immutable lineage evidence. It must not be used to execute the campaign.

Validation proved the S1-pinned leg3e deadline stale: calldata ending in deadline `0x6a74c23a` reverted with `TransactionDeadlinePassed`. S1b was created as the replacement fire-time stage with fresh fork-simulation inputs and a hard leg3e executor deadline of unix `1786124483` (2026-08-07 17:41:23 UTC).

S1b binding mechanics and packet hashes are valid, but its execution window separately expired before publication. See `S1B-RESOLUTION-LOG.md`. This note invalidates S1 for execution only; neither S1 nor S1b historical evidence is deleted or rewritten.
