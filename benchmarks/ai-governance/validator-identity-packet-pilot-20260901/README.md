# Validator identity-packet pilot

This is a one-validator proof that Corbanu Terminal exec can research a public
validator identity, return a fixed-heading Markdown packet, and preserve the
JSONL session log that produced it.

## Pilot subject

- network: XRP Ledger mainnet
- validator: `nHU4bLE3EmSqNwfL4AP1UZeTNPrSPPP6FXLKXo2uqfHuvBQxDVKd`
- frozen claimed domain: `ripple.com`
- frozen domain-verification status: `null`

The packet concludes that Ripple Labs Inc. is the most likely public identity,
adds a single 90–160 word neutral business-reference summary, and carefully
does **not** claim that the research proves current validator-key control.

## Generation boundary

The exact initial prompt is in `prompts/`. It was passed to
`corbanu --search exec` through stdin. Corbanu Terminal 0.1.36 used the
configured `gpt-5.6-sol` model, read-only sandbox, no approvals, and live web
search. Codex exec fallback was not used.

The final Markdown answer was written to `packets/`; the full Corbanu exec
JSONL event stream is in `logs/`. The empty stderr file is retained and
hashed. The log's final `agent_message` is byte-equivalent to the packet after
trimming terminal newlines.

## Files

- `inputs/validator.json`: exact frozen validator row
- `prompts/<validator>.txt`: exact initial prompt
- `packets/xrpl/<validator>.md`: generated identity packet
- `logs/xrpl/<validator>.jsonl`: Corbanu exec event log
- `logs/xrpl/<validator>.stderr.log`: captured stderr
- `manifest.json`: hashes, tool configuration, thread ID, and usage
- `run_one.sh`: process-reproduction command that requires a new output directory
- `verify_packet.py`: structural, hash, JSONL, and packet/log equivalence checks

## Verify

```bash
python3 verify_packet.py
```

To repeat the research process without overwriting the frozen evidence:

```bash
./run_one.sh /tmp/identity-packet-rerun
```

A rerun uses live web search and is not expected to reproduce the packet bytes.
Only the committed packet and hashes are the frozen downstream H200 input.

This packet is `SHADOW_ONLY`, external identity evidence. It is neither XRPL
consensus data nor a legitimacy/reputation score. The later H200 replay should
consume the frozen Markdown packet bytes, not rerun web research.
