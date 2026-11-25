# Public Launch Boundary

The source repository is public. The current network posture remains a
controlled-testnet implementation; publication does not promote the software to
mainnet or authorize real-value custody.

The devnet's Solana route holds real USDC in a capped vault on Solana
mainnet, and since 23 November 2026 pfUSDC can be withdrawn back to Solana. This is a
controlled devnet with small caps; there is no public announcement of Solana
withdrawals ([handoff of 25 November](../status/handoff-2026-11-25.md)).

## Controlled-Testnet Facts

- project-controlled validators are acceptable for protocol validation;
- reused machines are acceptable for controlled mechanics evidence;
- Cobalt mechanics can be green before public operator diversity exists;
- privacy-alpha can be exercised before public wallet/custodian rollout;
- write edges can remain controlled while read RPC is hardened.

## Public Launch Requirements

Public launch adds:

- independent validator placement evidence;
- stricter host/operator/legal-domain/funding-domain diversity;
- longer mixed transparent and shielded soaks;
- public write-edge abuse controls;
- external privacy and implementation review;
- custodian/exchange workflows;
- public benchmark packets for throughput claims.

Transactional indexed storage is implemented and active on the controlled
devnet; its G5 offline packet is bound to the October 21 binary, and G6 was
then open ([State And Storage](../architecture/state-and-storage.md)). Whether
either was repeated for the releases of 21 to 24 November is not recorded here.
A separate software signer process with a durable double-sign guard is
implemented on main; its deployment under two users is still open, and HSM
custody is not implemented ([Signer Policy](../runbooks/public-rpc-operator-policy.md#validator-signer-process)).
Production key custody, qualification of the exact deployed binary, independent
specialist review and deployment-specific recovery/fault drills remain
real-value requirements.

## How To Talk About It

Say what exists, cite evidence, and keep public-launch requirements in this
boundary page rather than repeating defensive caveats on every protocol page.
