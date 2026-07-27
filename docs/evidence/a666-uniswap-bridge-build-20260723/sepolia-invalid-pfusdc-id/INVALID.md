# Invalidated Sepolia predeployment

This zero-supply, zero-liquidity CONTROLLED deployment is not the a666 route.
Funding preflight proved that its PFTL settlement asset identifier was a stale,
nonexistent value (`02c46d...d7b`) rather than the live ce22 pfUSDC identifier
(`02c46a...05d7b`).

No bridge packet was consumed, no wA666 was minted, and no liquidity was
seeded. The contracts and initialized zero-liquidity pool are intentionally
abandoned. The sibling `sepolia-persistent/` evidence identifies the corrected
deployment.
