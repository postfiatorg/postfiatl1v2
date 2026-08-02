# NAV reserve proof fuzzing

These targets treat reserve witnesses, source evidence, and raw upstream
formats as attacker-controlled input. `witness_json` sends every successfully
decoded witness through the same deterministic `execute_reserve_proof` entry
point used by the SP1 guest, so all registered Aave, EVM spot, Hyperliquid,
NEAR, Solana, Monero, and Chainlink valuation dispatch paths are in scope.
`witness_cbor` exercises the exact SP1 guest input decoder, and
`source_evidence_json` isolates the tagged evidence decoder. Six additional
targets directly exercise the EVM, Hyperliquid, NEAR, Solana, Monero, and
source-checkpoint parsing surfaces through the production library, without
network access or state mutation.

Run the repository smoke campaign with cargo-fuzz 0.13.2 and the pinned
nightly toolchain:

```text
scripts/check-nav-reserve-proof-fuzz-smoke
```

The runner copies reviewed seeds into guarded temporary corpus directories.
Never pass a tracked fixture directory to libFuzzer as its first corpus: the
first corpus is writable, and doing so pollutes reviewed evidence with generated
inputs. Set `POSTFIAT_NAV_FUZZ_SECONDS` to change the per-target duration and
`POSTFIAT_NAV_FUZZ_TOOLCHAIN` only when reproducing with an equivalent local
nightly toolchain.

Production qualification requires retained corpora for every registered
adapter, a fixed-duration CI campaign, zero crashes/timeouts/OOMs, and checked-in
regression inputs for every finding. Merely compiling these targets does not
qualify an adapter.
