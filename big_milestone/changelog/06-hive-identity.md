# Changelog — Pillar 6 — Hive Chat Identity on Chain

How each reviewer problem was handled, round by round. The spec itself is [here](../pillars/06-hive-identity.md); the reviews are under [feedback](../feedback/README.md).

## Round 1 review → revision 1

Reviews: [round 1](../feedback/round-1/06-hive-identity.md).

| Reviewer item | Change |
|---|---|
| Gemini P1 · Opus P1 | ML-DSA hedging confirmed in the code (`try_sign` → `OsRng`). v2 now derives from the seed (R2, R3). The diagram no longer shows ML-DSA as the signer. |
| Gemini P1 · Opus P2 | Partly disputed: the current signer is the old-chain secp256k1 wallet using RFC 6979, which is deterministic, and no migration forces a new npub. v1 kept (R1, AC2), and `rotate_account` added for network changes. |
| Gemini P2 · Opus P7 | Handles now need registrar attestation, and existing handles are pre-attested. An unattested claim never overrides a Postgres binding. NIP-05 is stated as DNS-anchored (R11, R12). Open question 2 added. |
| Gemini P3 · Opus P3 · Opus P14 | Native object recommended, with the memo-size reason (R4). Payloads inline (R10), and BIP-340 checked in consensus (R8). Open questions 1 and 10. |
| Gemini P4 · Opus P11 | One identity object per account. Pillar 8 moved to "Used by". Open question 11. |
| Gemini P5 · Opus P4 | Operations table with `unlink` and `rotate_account` (R6). Signature digest scope defined (R7). Open question 4. |
| Opus P5 | Compromise cases and badge rules added (R18, Security model). Open question 7. |
| Gemini P6 · Opus P12 | Economics section added. Open questions 3 and 8. |
| Gemini P7 · Opus P10 | Old-chain link added, reused by Pillar 4 (R14). EVM and Solana proofs (R15). Avatar field (R5). |
| Gemini P8 | Kind-0 profile with NIP-39 tag (R17). Relays map in `nostr.json` (R12). Open question 9. |
| Gemini P9 | Single Rust v2 implementation with test vectors (R3, AC8). |
| Opus P6 | Quantum boundary stated, with an ML-DSA tag for high-value events (R20). |
| Opus P8 | Links opt-in, shielded addresses unlinkable, warning shown at mint (R16). |
| Opus P9 | Confirmed that the helper uses the consensus key. New link context, staleness on rotation and a reverse index (R13). Open question 6. |
| Opus P13 | `identity_kind` and `controller` fields. `hive-board@` minted as an agent (R19, AC9). |
| Opus P15 | Negative, migration and parity criteria added (AC2, AC4, AC7, AC8). |
| Gemini and Opus intent notes (native transaction weakened, Hive adaptation, other primitives, gating) | Native object recommended (R4). Hive adaptation covered by R17–R20. Avatar, agent and issuer fields added (R5). Gating offered as open question 8. |
| Cross-pillar (Pillars 4, 5) | `pq_receive_address` field (R5, R16). Browser wallet signing listed as a dependency. Old chain and new chain named explicitly. |

## Round 2 review → revision 2

Reviews: [round 2](../feedback/round-2/06-hive-identity.md).

| Item | Change |
|---|---|
| Gemini P1 · Opus P1 | `rotate_account` limited to one chain; its digest names both accounts, and the new account must have no record (R7, AC8). A network change is a fresh mint with the same npub and a re-attested handle (R25). Kind-0 carries one `i` tag per chain, merged; non-production builds do not publish kind-0 (R17, R25, AC14). Genesis spec checked: mainnet and token migration are out of scope, so no state import is assumed. |
| Gemini P2 | Handle lifecycle separated from npub reuse. Registrar `handle_reassign` with reason codes (R6, R24, AC12). Compromise section rewritten per case. npub and handle reuse after revoke or rotate is open question 4 (one consensus parameter per option). |
| Gemini P3 | `controller` is now a link co-signed by the controller account, which can unlink itself (R19). Negative test added (AC7). |
| Gemini P4 | Linked-key-only `unlink` is limited to consensus-verified links and checked against the key stored on the link. Any funded account submits and pays; the digest binds `link_id`, not the owner's sequence (R6, R7, R13). Verified in code: the chain has no fee-payer field, so the signer always pays. |
| Gemini P5 | One active link per `node_id` and per old-chain account, matching Postgres migration 156's unique indexes (R9, AC7). Staleness is computed at read time (R13). |
| Gemini P6 | v1 in Task Node Local and the CLI: import the mnemonic plus the exported `accountId`, or the raw Nostr secret. The `accountId` stays off chain (R1, AC10). |
| Gemini P7 | `key_index` added to the v2 KDF and stored on the object (R2, R5, AC9). |
| Gemini P8 | `rotate` takes a reason. A compromised rotation strips badges like a revoke. A planned one shows only *previous key* (R6, R18, AC6). |
| Gemini P9 | Sizes checked in code: the largest operation is about 14 KB raw, under the per-type RPC caps; each signature fits the 8,192-character field; `identity_set` gets its own cap (R26). Field bounds added from existing limits: 5 relays as Task Node allows, the 256-byte text limit, a suite-sized `message_key` (R5). Proofs stay in history, not state. Fees now follow the chain's existing byte-weighted fee and burned state-expansion fee; the values are open question 3. Onboarding uses the starter-PFT faucet that already exists in `WalletHome.jsx`. |
| Gemini P10 | R20 redesigned. The signature cannot sit inside the event it signs, so it travels in the Pillar 1 pinned bundle or a companion event, never as a large relay tag. Running a relay stays open question 9 (a founder decision); the reasons it does not block were added. |
| Gemini P11 · Opus P2 | Registrar attestations now cover only the name (subject, handle, expiry), not the full payload. Existing bindings get on-chain `handle_reserve` entries keyed to an npub hash, so the registrar can be offline at mint (R7, R11). The contradictory "unattested claim" sentence in R12 was replaced. Canonical handle rule moved into consensus from `taskNodeNostrName` (R11). |
| Gemini P12 | Old-chain link format specified: compressed secp256k1 key, strict low-S DER ECDSA over SHA-512Half, account id from RIPEMD-160 of SHA-256 (R14). Pure-Rust library shared with `wallet_wasm` (R8). "Shipped first" removed; ordering is in Build order. |
| Gemini P13 · Opus P4 | Build order added (Steps 1–3, each mapped to acceptance criteria). R10 no longer makes the explorer wait on Pillar 2 schema ids. Pillar 8 boundary stated from Pillar 8 R6–R7: credentials are separate records and `did:pftl` includes the identity. The old DID open question was dropped as already answered by Pillar 8. Partly disputed: the brief forbids picking founder answers, so open questions get no defaults. Each is instead designed to work under every option. |
| Opus P3 | Verified in code. The derivation signature never leaves `deriveNostrMessagingIdentity`. But `signedCollaborationProof` signs any server-supplied text with the same deterministic key, so a compromised server could obtain the v1 seed input. Added the signing guard (R1, AC13), a new "What exists" row and a reframed open question 5. |
| Opus P5 | Key-type byte added; ed25519 old-chain keys are part of open question 10. Pillar 4 checked: its burn-memo claim path works for any key type and does not depend on this link, so the page no longer claims Pillar 4 reuses it (R14, Used by). |
| Opus P6 | Namespace registry modelled on `NavAttestorRegisterOperation`: register, rotate, remove, compromise height, reason codes. The 365-day expiry is not carried over (R24). Who governs it is open question 2. |
| Opus P7 | Economics rewritten on the chain's real fee model (byte-weighted fee, burned state-expansion fee, account reserve). The previous "no owner reserve" framing missed this. The values and sponsorship stay open question 3. |
| Opus P8 | Open question 11 added: should the identity also be a non-transferable NFT. Designed so either answer adds at most one step to R17. |
| Opus P9 | Compromise cases split: a leaked derived account key does not reveal the seed or a v2 Nostr key; a seed compromise does. |
| Opus P10 | Canonical handle rule enforced in consensus; non-canonical input is rejected, not normalised (R11, AC7). |
| Opus P11 | Mint flow unlocks the old-chain wallet for v1 and signs the old-chain link in the same step (R17, story steps 1 and 4). |
| Opus P12 | Events anchored on chain before a revoke (Pillar 1 transcripts) or carrying an R20 signature keep an *anchored before revoke* label (R18). |
| Opus P13 | Secp256k1 scope stated: identity binding only, never value. A governance amendment can switch off new BIP-340 operations (R8). |
| Opus P14 | EVM and Solana links pay the link fee, count toward the cap, keep only compact records in state and move to Step 3 (R15, R26). |
| Code corrections found while revising | Memo limit is 512 bytes in total, not 4 × 256. Workspace does contain `k256` and `ed25519-dalek`, but only in the `arc-conformance` test crate, so the status now says "in the consensus crates". |
| Cross-pillar (Pillar 10 R24) | Hive-mind attribution may use the minted identity or a Task Node-only pseudonym, and open question 8 no longer lists hive-mind credit as gateable (R19, Economics, open question 8). |
| Cross-pillar (Pillar 3 R13, R15) | The validator link now depends on Pillar 3 R13's connect command and its signer allow-list, which must include `postfiat-l1-v2/identity-link/v1`. R13 notes that Task Node's binding follows a ratified rotation by validator id (Pillar 3 R15), while the on-chain link goes stale until re-linked. Open question 6 now points to Pillar 3's matching question. |
| Cross-pillar (Pillar 2 R9) | `message_key` reference confirmed as Pillar 2 R9. The master-seed compromise case now notes that this seed-derived key has no forward secrecy. |
| Cross-pillar (Pillar 1) | Verified in Task Node: the bundled WASM exports `wallet_sign_payment_v2`, but `tx-builder.js` never calls it, and no `identity_set` signer exists. The Dependencies row and R17 now list what is missing: the `identity_set` type, a `wallet_sign_identity_set_fields` export, a rebuilt WASM bundle, a `tx-builder.js` branch and its submit RPC. |
| Length | Code references now list only files not already linked in the tables. |
