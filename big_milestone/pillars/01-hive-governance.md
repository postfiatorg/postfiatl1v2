# Pillar 1 — Hive Chat Governance Module

> **Status:** Partial. Hive Chat, IPFS pinning, and pointer anchoring already exist in Task Node. What still needs to be built: the automatic protocol-state snapshot, the founder-triggered 30-minute governance session room, signed minutes with carry-over accountability, new-chain anchoring, and the explorer + public archive view.
>
> **Outcome:** [A · Post-human governance in public](../README.md#three-outcomes) — the public place where anyone can watch Post Fiat govern itself.

## What this pillar is (in plain English)

Post Fiat governance runs as a visible loop of **information packets** and **public accountability**:

1. **Whenever you kick off a session in Task Node**, a script automatically pulls a live **protocol snapshot** (chain height, active validators, bridge/NAVCoin supply, Task Node health) and **automatically loads the deliverables from the previous governance session** so everyone can see what actually got done and what didn't.
2. **You add a short written brief** — what happened since last time, what you did or didn't finish from the last session, the questions to discuss, and any proposed decisions — then sign it with your wallet.
3. **Task Node publishes the packet** to **IPFS** *(a content-addressed file network where every file gets a permanent fingerprint called a CID; if one byte changes, the CID changes)* and writes that CID into a tiny signed transaction on the **new chain (`postfiatl1v2`)** *(called "anchoring" — it proves the exact packet existed at that block and cannot be silently edited later)*.
4. **A 30-minute public governance session** opens in its own Hive Chat room (so normal Hive Chat keeps running uninterrupted). You choose who can post (just you, an allowlist like validators, or all Hive members) and how active the AI facilitator is. The facilitator walks through the agenda and asks structured questions; you answer inline.
5. **When the session closes**, Task Node freezes the exact ordered transcript and drafts **minutes + next deliverables**. You review, edit if needed, and sign off with your wallet. Both the frozen transcript and signed minutes are pinned to IPFS and anchored on `postfiatl1v2`.
6. **Follow-through happens automatically:** There are **no automatic PFT bounties** generated from minutes. Instead, the next time you kick off a governance session, the previous session's deliverables are loaded straight into the opening packet — **either you did the work or you didn't**, in public. If you explicitly flag a decision for **deterministic governance**, it is also sent to the deterministic scoring/ruling runner (on our current Google setup or Vast on demand) and the replayable ruling is anchored alongside the session.

## How a session runs end-to-end

```mermaid
sequenceDiagram
    autonumber
    actor F as Founder
    participant C as Snapshot Collector
    participant S as Task Node
    participant I as IPFS
    participant L as postfiatl1v2
    participant H as Session Room (Hive)
    participant A as AI Facilitator
    participant D as Deterministic Runner (Google / Vast)
    F->>S: Click "Start Governance Session"
    C->>S: Live chain/validator/bridge snapshot + previous session deliverables
    F->>S: Mark previous items done/not done + write brief & agenda
    S->>I: Pin packet & snapshot (permanent CID)
    F->>L: Sign anchor tx on postfiatl1v2
    S->>H: Announce session & open dedicated room with founder toggles
    loop ~30 minutes
        A->>H: Ask structured agenda / follow-up question
        F->>H: Answer inline (community posts if policy allows)
    end
    F->>S: Close session (or 30-min timer ends)
    S->>S: Freeze ordered transcript bundle
    A->>S: Draft minutes & next deliverables (citing exact message IDs)
    F->>S: Review, edit & sign approval
    S->>I: Pin transcript bundle & signed minutes
    F->>L: Sign anchor txs on postfiatl1v2
    opt Only for decisions explicitly flagged by founder
        S->>D: Run deterministic ruling with snapshot as evidence
        D->>L: Pin & anchor replayable ruling
    end
```

## Locked-in design decisions

- **Trigger:** Sessions run **on demand whenever the founder starts one in Task Node** (no rigid calendar deadline and no fake "missed session" failure state).
- **Authority & approval:** **Founder-only (1-of-1)** opens sessions and signs minutes at launch. The approval record still carries a signer-set ID and threshold field so multi-sig stewards can be added later without changing the schema.
- **Anchor chain:** **New chain (`postfiatl1v2`)** only, using standard `payment_v2` memos so the new block explorer displays governance sessions natively and we carry zero old-chain migration debt.
- **Follow-through mechanism:** **Direct carry-over, not PFT bounties.** Every new governance packet automatically instantiates with the previous session's deliverables list, forcing an explicit public status (`done`, `in_progress`, `not_done`, `dropped`) on each item.
- **Deterministic rulings:** Run **only when explicitly flagged** on a specific decision by the founder, using the **current Google setup or Vast** on demand. The chat facilitator and minutes drafter run on Task Node's normal inference stack with **zero GPU rental**.

## What exists today

Checked against Task Node `main` (`204d238`) and `postfiatl1v2`:

| Piece | Current state | Where |
|---|---|---|
| Hive Chat room (Nostr) | Live, but hardcoded to one room (`tasknode-hive-v1`); DB table already supports multiple channels | [`shared/hive-group.js`](https://github.com/postfiatorg/tasknode/blob/main/shared/hive-group.js), [`HiveGroupChat.jsx`](https://github.com/postfiatorg/tasknode/blob/main/src/features/hive/HiveGroupChat.jsx) |
| Room API & relay sync | Live, public read, signed write. Relay sync currently only looks back ~10 minutes, which needs widening for session rooms | [`server/hive-group-routes.js`](https://github.com/postfiatorg/tasknode/blob/main/server/hive-group-routes.js), [`server/hive-group-worker.js`](https://github.com/postfiatorg/tasknode/blob/main/server/hive-group-worker.js) |
| Hive AI bot | Live, but polls once every 60 seconds — too slow for a live 30-minute session | [`server/hive-group-provider.js`](https://github.com/postfiatorg/tasknode/blob/main/server/hive-group-provider.js) |
| IPFS pinning | Live (Pinata + first-party cluster, 16 MiB cap), but uses plain `JSON.stringify` instead of canonical deterministic JSON | [`server/context-ipfs.js`](https://github.com/postfiatorg/tasknode/blob/main/server/context-ipfs.js) |
| Anchoring (`pf.ptr` v4) | Live on the **old chain** only. New chain (`postfiatl1v2`) already supports memos (up to 4 memos, 256 bytes each), but Task Node doesn't sign new-chain memos yet | [`server/pftl-pointer.js`](https://github.com/postfiatorg/tasknode/blob/main/server/pftl-pointer.js), [`core_chain.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/types/src/core_chain.rs) |
| Block explorer governance view | Not built yet | [`docs/explorer-spec.md`](https://github.com/postfiatorg/postfiatorg.github.io/blob/main/docs/explorer-spec.md) |
| Deterministic governance runner | Exists in shadow/research mode (Verifiable Constitution + Cobalt); runs on Google setup or Vast on demand | [`docs/governance/`](https://github.com/postfiatorg/postfiatl1v2/tree/main/docs/governance) |

## Detailed specification

### 1. Protocol-state snapshot & carry-over packet
- **Automatic collector (`pf-gov-collector`):** Generates `pf.governance.snapshot.v1`. Every field stores its exact source (RPC call, API endpoint, or repo commit), timestamp/block height, and SHA-256 hash of the raw response. Raw responses are pinned alongside the snapshot so any third party can re-verify every number.
- **What the snapshot pulls automatically:**
  - `postfiatl1v2` status, active validator set, ledger tip, bridge status, and NAVCoin supply status.
  - Current Cobalt status and latest dynamic-UNL scoring verdict.
  - Task Node `/api/system/status`.
  - **Previous session's deliverables list** (loaded automatically from the last published governance minutes so each item must be marked `done`, `in_progress`, `not_done`, or `dropped`).
- **Founder brief (`pf.governance.packet.v1`):** Attaches the read-only snapshot by CID + hash, plus your editable brief: `title`, `summary`, `previous_deliverables_status[]`, `open_questions[]`, `proposed_decisions[]`, and optional IPFS attachments (text/Markdown/JSON only for agents).
- **Canonical JSON:** Before pinning, Task Node sorts keys deterministically (RFC 8785 / JCS) so the same packet always produces the exact same SHA-256 hash and IPFS CID.

### 2. Dedicated session room & live facilitation
- **One room per session:** Starting a session creates a new `hive_group_channels` room and posts an announcement card in the main Hive room (main chat is never locked).
- **Session-open toggles** (signed by your Nostr key):
  - **Who can post:** `founder_only`, `allowlist` (e.g., validators), or `all_members`.
  - **AI facilitator mode:** `off`, `agenda` (walks through the packet's questions in order), or `interactive` (also asks follow-ups).
  - **Session length:** 30 minutes default (founder can extend or close early).
- **Anti-spam & engineering defaults:**
  - Posting rate limit: 1 message per 15 seconds per pubkey (founder exempt).
  - AI facilitator latency target: under 10 seconds per reply (event-driven on new messages, replacing the old 60-second poll loop).
  - AI facilitator turn cap: max 25 turns per 30-minute session (bounded inference cost, zero GPU rental).
  - Relay ingest starts at session-open time so a brief Task Node hiccup never drops messages.
- **Prompt-injection boundary:** The AI prompt separates the packet, founder messages, and community messages into distinct blocks. Community text is always treated as quoted data, never as instructions, and the validator rejects any drafted decision or deliverable that doesn't cite a founder or agenda message ID.

### 3. Frozen transcript, signed minutes & anchoring
- **Canonical inclusion rule:** When the session freezes, a message is included in the official transcript only if its Nostr signature is valid, it targets the session room, the author was allowed by the session toggle, and Task Node received it between open and freeze within the rate limit. Ordering uses Task Node's server receipt sequence (so users cannot backdate timestamps). Excluded messages are listed by ID with reason codes.
- **Self-contained bundle:** Task Node pins the transcript manifest, every raw signed Nostr event, and a readable Markdown transcript to IPFS so history never depends on third-party Nostr relays staying online.
- **Minutes (`pf.governance.minutes.v1`):** Four clean sections — `previous_deliverables_review[]`, `decisions[]`, `next_deliverables[]`, and `community_input[]`. Every item is labeled `human`, `agent`, or `deterministic` and cites exact message IDs.
- **Founder approval & new-chain anchoring:** You edit the draft if needed and sign the approval with your wallet (the server never signs on your behalf). Task Node verifies every CID is readable on at least two IPFS gateways, then your wallet submits idempotent `pf.ptr` v4 memos (`kind = GOVERNANCE` / `12`, grouped by `thread_id = session_id`, fitting inside `postfiatl1v2`'s 256-byte memo limit) on `postfiatl1v2`. An anchor is marked complete only after an **accepted** chain receipt.
- **Optional deterministic ruling:** Only decisions you explicitly check as `run_deterministic_ruling` are exported with the snapshot to the deterministic governance runner (Google setup or Vast on demand). The resulting ruling + replay hashes are pinned to IPFS and anchored to the same `session_id`.

### 4. Public archive, verifier & block explorer
- **Public session page (no login required):** Shows the live or completed session, previous-session scorecard, every pinned artifact with CID + `postfiatl1v2` transaction link, facilitator latency stats, and a **Verify** link.
- **Independent verifier script (`verify-governance-session`):** Anyone can run it using only public IPFS gateways and `postfiatl1v2` RPC to check every signature, hash, snapshot raw-response derivation, transcript inclusion rule, and ruling replay hash.
- **Block explorer view:** Groups `GOVERNANCE` memos by `thread_id` so each governance session renders as a clean timeline directly on the explorer.

## Security summary

- **Fake or backdated chat messages:** Blocked by Nostr signature checks + server receipt ordering; direct-to-relay bypasses are filtered by the inclusion rule and logged as exclusions.
- **Relay loss or server tampering:** Every raw signed event lives in the IPFS transcript bundle, and your wallet signs the chain anchors over the canonical hashes.
- **Prompt injection into minutes:** Community messages are isolated as quoted data; drafted decisions/deliverables must cite founder/agenda messages and always require your explicit signature.

## Milestones & completion checklist

### Build milestones
- [ ] **M1 · Multi-channel Hive & canonical IPFS:** Support multiple `hive_group_channels` in routes/worker/UI, widen session relay ingest to session-open time, and add deterministic RFC 8785 canonical JSON mode to `pinContextIpfsJson`.
- [ ] **M2 · Protocol-state collector & carry-over:** Build `pf-gov-collector` (`pf.governance.snapshot.v1`) pulling live chain/validator/bridge/Task Node state plus automatically loading the previous session's deliverables for done/not-done scoring.
- [ ] **M3 · Session UX & fast AI facilitator:** Build the Task Node "Start Governance Session" flow (brief editor, toggles, 30-min room, freeze button) and an event-driven facilitator (`<10s` reply target, 25-turn cap, prompt-injection guardrails).
- [ ] **M4 · Transcript freeze, minutes & `postfiatl1v2` anchoring:** Build transcript bundling, minutes drafting/validation, wallet-signed approval, and idempotent `pf.ptr` v4 (`GOVERNANCE`) memo anchoring on `postfiatl1v2` (requires new-chain memo signing from [Pillar 2](02-message-protocol.md)).
- [ ] **M5 · Public archive, explorer view & verifier:** Ship the no-login public session page + archive, the `postfiatl1v2` explorer governance timeline, the standalone `verify-governance-session` script, and optional flagged-decision export to the deterministic runner (Google setup / Vast).
- [ ] **M6 · Adversarial red-team check:** Verify prompt-injection resistance (community text cannot inject decisions/deliverables) and relay-bypass exclusion tests.

### Required ship gates
- [ ] **1. Executed:** All M1–M6 code merged and deployed to production Task Node, `postfiatl1v2` explorer, and public archive.
- [ ] **2. Tested by user:** Founder kicks off and completes a full live 30-minute governance session in production (including carry-over from a prior packet, signed minutes anchored on `postfiatl1v2`, third-party `verify-governance-session` pass, and at least one flagged deterministic ruling).
- [ ] **3. Added to docs:** Governance packet schema, snapshot methodology, and verification guide published in the Post Fiat docs.
- [ ] **4. Public blog post document:** Public blog post written, published, and linked from the session page explaining Post Fiat's post-human governance loop, packet schema, and how anyone can audit a session.

## Code references

- Hive Chat: [`shared/hive-group.js`](https://github.com/postfiatorg/tasknode/blob/main/shared/hive-group.js) · [`server/hive-group-worker.js`](https://github.com/postfiatorg/tasknode/blob/main/server/hive-group-worker.js) · [`server/hive-group-routes.js`](https://github.com/postfiatorg/tasknode/blob/main/server/hive-group-routes.js) · [`HiveGroupChat.jsx`](https://github.com/postfiatorg/tasknode/blob/main/src/features/hive/HiveGroupChat.jsx)
- Pointers & IPFS: [`server/pftl-pointer.js`](https://github.com/postfiatorg/tasknode/blob/main/server/pftl-pointer.js) · [`server/context-publish.js`](https://github.com/postfiatorg/tasknode/blob/main/server/context-publish.js) · [`server/context-ipfs.js`](https://github.com/postfiatorg/tasknode/blob/main/server/context-ipfs.js)
- L1 & Governance: [`core_chain.rs`](https://github.com/postfiatorg/postfiatl1v2/blob/main/crates/types/src/core_chain.rs) · [`docs/governance/`](https://github.com/postfiatorg/postfiatl1v2/tree/main/docs/governance) · [`docs/explorer-spec.md`](https://github.com/postfiatorg/postfiatorg.github.io/blob/main/docs/explorer-spec.md)
