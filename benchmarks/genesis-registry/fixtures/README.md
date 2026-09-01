# Genesis-registry fixtures

Golden vectors and one-field-mutation rejection fixtures for the canonical
`ProposedGenesisRegistryV1` types (`crates/types/src/genesis_registry.rs`),
implementing work-sequence step 1 of
`docs/architecture/genesis-registry-proposal-path.md`. Everything here is
`SHADOW_ONLY` rehearsal data: it grants no authority and touches no network.

## Provenance

Built entirely from the archived fork rounds 12-19 in
`benchmarks/ai-governance/dunl-subscorer-shadow-20260901/rounds/` (verified
against `rounds-manifest.json`; never refetched). Regenerate with:

```
GENESIS_REGISTRY_FIXTURES_REGEN=1 cargo test -p postfiat-types \
    regenerate_genesis_registry_fixtures
```

The Python reference implementation (`python/postfiat_rpc/genesis_registry.py`)
must reproduce every `proposed_registry_hash_hex` from the same inputs —
two-implementation hash agreement is the acceptance bar.

## Derivation rules for archive-absent artifacts

The archived subset omits `bundle.json`, `outputs/final_scores.json`, the
convergence report, the anchor transaction, and all identity receipts. The
fixture builder derives deterministic stand-ins (fixture-only; a real genesis
round supplies all of these as frozen artifacts):

| Field | Fixture rule (round `N`, id `testnet-rN`) |
| --- | --- |
| `chain_id` | `postfiat-l1v2-testnet` |
| `bundle_cid` | `fixture:dunl-subscorer-shadow-20260901/testnet-rN` |
| `bundle_digest` | SHA-256 of the compact sorted-keys JSON map `{relative_path: sha256_hex}` over the round's seven archived files |
| `manifest_digest` | SHA-256 of `runtime/execution_manifest.json` bytes |
| `final_scores_digest` | SHA-256 of compact sorted-keys JSON `{"final_scores": [{"final_score", "master_key"}, ...]}` over all scored validators sorted by master key, scores recomputed with the pinned formula `min((50c+20r+10s+10d+10i)//100, c+25)` |
| `selected_unl_digest` | SHA-256 of `outputs/selected_unl.json` bytes |
| `convergence_report_digest` | SHA-256 of ASCII `fixture-convergence:testnet-rN` |
| `anchor_tx_hash` | SHA-256 of ASCII `fixture-anchor:testnet-rN` |
| `receipt_deadline` | hash: SHA-256 of ASCII `fixture-receipt-deadline:testnet-rN`; seq: `90000000 + 1000*N` |
| receipt `expiry_close_time` | deadline seq + 86400 |
| receipt ML-DSA key | 61 concatenated blocks `SHA-256("L1V2_GR_FIXTURE_MLDSA_V1" \|\| master_key_33 \|\| u32_be(i))`, 1952 bytes |

Identity-evidence records (domain, verified-domain status, provider, country)
come from the frozen `inputs/model_request.json` VALIDATOR DATA array joined
through `inputs/validator_map.json`. Final scores and cutoff (40) come from
`outputs/validator_scores.json` and `runtime/execution_manifest.json`.

## Layout

- `golden/testnet-rN.json` — registry JSON, canonical CBOR hex, and
  `proposed_registry_hash_hex` (`digest("L1V2_PROPOSED_GENESIS_REGISTRY_V1", ...)`).
- `receipts/testnet-rN.json` — the fixture receipt set (`Receipted`). Rounds
  12-18 receipt every selected operator; round 19 omits the last two selected
  operators, so its registry carries 18 entries and exercises the
  `Selected ∩ Receipted` intersection.
- `mutations/testnet-r12/*.json` — 25 one-field mutations of the round-12
  canonical encoding; each must be rejected with the named `expected_error`
  by both implementations.
