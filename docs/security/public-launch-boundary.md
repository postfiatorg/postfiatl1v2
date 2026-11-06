# Public Launch Boundary

The source repository is public. The current network posture remains a
controlled-testnet implementation; publication does not promote the software to
mainnet or authorize real-value custody.

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
devnet; its G5 offline packet is bound to the October 21 binary, while G6
remains open ([State And Storage](../architecture/state-and-storage.md)).
A separate software signer process is implemented on main, with its own
double-sign guard and deployment qualification still open; HSM custody is not
implemented ([Signer Policy](../runbooks/public-rpc-operator-policy.md#validator-signer-process)).
Production key custody, qualification of the exact deployed binary, independent
specialist review and deployment-specific recovery/fault drills remain
real-value requirements.

## How To Talk About It

Say what exists, cite evidence, and keep public-launch requirements in this
boundary page rather than repeating defensive caveats on every protocol page.
