# Burn 2 repair review — 2026-09-10

Status: findings recorded; P1/P2 repair pending

Reviewed checkout: `b34adf20a809b991754fdc09111827e7577b6b2e`

## Scope and method

This fresh-eyes pass reconstructed the four Burn 1 repairs from their finding
documents and source diffs, then inspected the current implementations and
sibling instances of the signing-race, refund-path, connection-exhaustion, and
evidence-path defect classes. Frozen simulation, gate, and deployment evidence
was read only. The pass did not contact a fleet host, mutate a chain, or use
Task Node.

The repaired Arc refund and execution-interlock paths close their source
findings. Consensus prepare, precommit, and timeout signatures are constructed
and verified while the per-height safety guard remains held; no sibling vote
signing path bypasses that critical section. UNL V2 now requires an incoming
post-epoch vouch and post-epoch co-work, keeps stale and fresh duplicate domains
separate, and renders root-valid untrusted fields as inert code spans. The
hypothetical-state helper remains the already-recorded P3 `UNL-04`.

## Findings

### 1. P1 — A forged forwarded authority still releases the loopback session token

**Location:** `wallet-proxy/server.js:203-224`.

The Burn 1 repair accepts `X-Forwarded-Host` in preference to the HTTP
`Host` header. A browser can supply `X-Forwarded-Host` itself; the endpoint
does not authenticate a trusted reverse proxy or require both authorities to
be allowed. The direct request therefore retains an attacker-controlled way
to replace the authority that the DNS-rebinding check was intended to bind.

**Concrete failure scenario:** script loaded from `https://attacker.example`
rebinds that name to loopback, sends `Host: attacker.example`,
`X-Forwarded-Host: localhost`, and `Sec-Fetch-Site: same-origin`. The
current helper selects only `localhost`, returns true, and the local-session
response discloses the mutation bearer token. The existing regression covers a
host-only rebinding attempt, not this forwarded-host variant.

### 2. P1 — The exported WASM transfer signer remains quote-authoritative

**Location:** `crates/wallet_wasm/src/lib.rs:251-286`.

The browser call sites added by Burn 1 compare a quote with reviewed fields, but
the exported `wallet_sign_transfer(backup_json, quote_json)` API still accepts
no request or intent argument. It checks only that the quote sender equals the
wallet address, then signs the quote's recipient and amount. The original
finding explicitly included this shipped WASM signing boundary, so caller-side
checks do not fully close it.

**Concrete failure scenario:** a direct consumer of the maintained WASM package
asks an RPC for a transfer to account A, receives a structurally valid quote
for the same wallet but account B and a larger amount, and passes that quote to
the public two-argument signer. The function signs account B and the larger
amount. Both current browser applications are protected by their own checks,
but the exported signing API is still unsafe when used as documented.

### 3. P1 — Finite RPC exhaustion exits cleanly under an on-failure-only supervisor

**Location:** `crates/node/src/rpc_cli.rs:688-820`,
`crates/node/src/batch_snapshot.rs:2370-2390`, and
`systemd/postfiat-rpc.service.example:13-17`.

The RPC serve loop intentionally stops accepting after `--max-requests`,
drains active keep-alive connections, and returns a successful report. Both
the generated validator RPC unit and the maintained service example specify
`Restart=on-failure`. This contradicts the operator policy that systemd
rotates the bounded process after clean exhaustion and preserves the diagnosed
`RPC-01` recurrence.

**Concrete failure scenario:** after 10,000 accepted connections, a persistent
client eventually closes or reaches its read deadline. The RPC process then
returns normally with exit status zero. Systemd does not restart an
`on-failure` service after that clean exit, so the endpoint remains down.
A continuously active keep-alive client can first extend the no-accept window,
which is the exact validator-0 failure shape already reproduced in the
read-only diagnosis.

## No additional P1/P2 findings

- The V2 controller's post-deadline cancellation validates the same finalized
  receipt and writes the same three replay fences as mint consumption. The PFTL
  refund verifier binds the route, packet digest, domain-separated source
  packet, deadline, and finalized external event.
- The top-level A666 mainnet round command checks both an execution flag and an
  exact intent phrase before credentials, endpoints, or network setup.
- Consensus v2 has one production signer for each prepare, precommit, and
  timeout vote. Each now signs inside its authorization callback; callback
  failure burns a durable authorization conservatively.
- UNL V2 relation signatures bind both accounts, current control epochs,
  statement, effective/expiry windows, and relation direction. The fresh-score
  duplicate fix is order-independent for stale and current rows.

## Pre-repair verification

- Arc V2 controller: **9 passed**; mainnet command interlock: **2 passed**.
- Consensus v2 libraries: **20 passed**; node binary selection: **3 passed**.
- Bridge, execution, and pfUSDC proof libraries: **38 + 196 + 5 passed**.
- Wallet proxy: **36/36 passed**; web wallet: **260/260 passed**; extension:
  **2/2 passed**.
- Rust RPC SDK and wallet WASM: **69 passed** across their test targets.
- Combined Task Node UNL V1/V2: **171 passed, 46 subtests passed**.

These green baselines do not exercise the three adversarial cases above. Burn 2
will repair all three with minimal source changes and focused regressions.
