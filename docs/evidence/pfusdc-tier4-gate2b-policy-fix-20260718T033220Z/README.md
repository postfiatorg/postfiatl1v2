# pfUSDC Tier-4 Gate 2B — Registrable ingress policy freeze

This record supersedes the ingress artifact in
`pfusdc-tier4-gate2b-20260718T030608Z`. Manifest/bootstrap validation found that
the old guest returned a 48-byte SHA3-384 policy string while the existing SP1
route and NAV profile contract requires exactly 32 bytes. The old artifact could
not be registered in a valid Tier-4 NAV profile and is invalidated.

Commit `5cafb31e642dfd1629f3a841ca7dc495e5d39668` changes only the ingress policy
commitment algorithm to domain-separated Keccak-256, adds a fixed conformance
vector, and extends the existing bootstrap bundle command to carry the exact
Tier-4 SP1/profile fields. The replacement guest was built once after that
source commit. No SP1 proof was generated.

- SP1 SDK: `6.3.1`
- Ingress ELF SHA-256:
  `f61cb50d07eb9f588b0d12d0ba74842fdaa39064f4f9a286e50c8c5be4198e1e`
- Ingress program vkey:
  `0x007a73f6c1661a43924e5f7212b75d2069943b20e96a475a2d101245977b5bb7`
- Egress ELF/vkey: unchanged
- Superseded ingress ELF SHA-256:
  `03e6b9dabf559f5bc69b8c4b501d31a45ab7db049f6d1d64a4a6e49edcc548eb`
- Superseded ingress program vkey:
  `0x007b629db1f140ba592d36ed9ec62ab807d78ecc292fa0b435c9f7f180238df4`

The tracked replacement ELF is byte-identical to Cargo's final RISC-V release
artifact. The fixed hash vector was independently calculated with Foundry
`cast keccak` over the canonical field preimage.
