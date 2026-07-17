# Redaction Policy

The hosted docs must not publish secret material.

## Never Publish

- private keys;
- mnemonic material;
- validator-private launch material;
- live credentials;
- raw SSH inventories;
- private Orchard witness material;
- spending keys;
- full viewing keys;
- note seeds;
- Merkle auth paths;
- `reports/testnet-private-key-material/`;
- raw `master_seed_hex`, `spending_key_hex`, `full_viewing_key_hex`, or `rseed`
  fields.

## Guardrail

Run:

```bash
scripts/docs-site-redaction-check
```

The script scans hosted docs and fails on known private-material patterns. This
page is excluded from that scan because it deliberately names the forbidden
fields.

## Evidence Policy

Only redaction-safe reports should appear in the hosted evidence index. The
site should never auto-publish the entire `reports/` tree.
