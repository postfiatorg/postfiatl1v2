# pfUSDC Tier-4 Gate 2B — Frozen V3 ingress program

This evidence freezes the single V3 ingress guest build required before route
and deployment hashing. It is build/setup evidence only. No SP1 proof was
generated.

- Guest source freeze: `0b68a5be71c80d1cdc89d12e5c7cfe77b1eb831f`
- SP1 SDK: `6.3.1`
- Ingress ELF SHA-256:
  `03e6b9dabf559f5bc69b8c4b501d31a45ab7db049f6d1d64a4a6e49edcc548eb`
- Ingress program vkey:
  `0x007b629db1f140ba592d36ed9ec62ab807d78ecc292fa0b435c9f7f180238df4`
- Egress ELF/vkey remained unchanged.

The copied tracked ELF matched the final Cargo RISC-V release artifact byte for
byte. `program-info` derived the vkeys from the exact tracked ELFs. The first
host setup attempt failed before setup because `protoc` was not on `PATH`; the
same command succeeded with the pinned local `protoc` path. That failure did not
build or generate a proof.
