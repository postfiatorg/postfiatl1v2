# StakeHub PR #8 review — 2026-09-07

Status: independent read-only review

PR: [postfiatorg/StakeHub #8](https://github.com/postfiatorg/StakeHub/pull/8)

Reviewed base: `master` at `cdf792929f8d2b37cf3fdf0b2050c183e96a0683`

Reviewed head: `handoff/navcoin-local-20260907` at `8f6f27cf66896b48f9a645e995a1288fc5f7218d`

**Merge recommendation: merge after fixes — the agent-policy bypass, live-on-invocation scripts, and remote secret-cleanup gaps are blockers; the false-success, RPC, signer-surface, archive, and amount-precision findings should be closed or explicitly dispositioned in the same update.**

## Scope and method

I reviewed the complete nine-commit PR diff against StakeHub `master`: 347
files, 341,236 insertions, and 10 deletions. The review covered every changed
path, with detailed inspection of the NAVCoin runner, deposit and checkpoint
paths; the agent and CLI signer changes; all new `shielded_exit_*` modules;
the live pfETH, deployment, probe, and fleet scripts; tests; documentation; and
the recovery archive. I used an isolated StakeHub checkout and an isolated
sibling checkout of current L1 `main`. The existing StakeHub checkout remained
on `master` with its pre-existing modified `AGENTS.md` untouched.

The named consolidation handoff was read from its separate local checkout, and
the machine handoff was read from L1 PR #39 head
`f2e749a16b07be446cb11da1e1f523eb9cdabca7`. I did not use Task Node, change
StakeHub source, switch its existing checkout, post to GitHub, approve the PR,
use a wallet, contact a live chain, or sign or submit a transaction.

Checks included:

- exact base-to-head diff inspection and `git diff --check`;
- signer, wallet, transaction submission, live-default, retry, error-path, and
  endpoint searches followed by source inspection;
- Gitleaks 8.30.1 over all nine commits plus targeted secret/key filename and
  literal scans; all 796 generic-key candidates were public token addresses,
  program verification keys, or other public chain identifiers;
- hash verification of all 190 entries in
  `docs/handoffs/navcoin-recovery-20260907/resumed-epoch10/inventory.json`;
- syscall tracing of the PR-added Python tests to identify network destinations;
- a clean sibling-layout Rust build against current L1 `main`; and
- fresh Python and strict documentation checks in scratch directories.

## Verified build and tests

- **Relative Cargo dependency: pass.** In a clean scratch layout named
  `StakeHub/` and `postfiatl1v2/`, L1 was at
  `5261a8b89c8a31ecdbfff465d2dd64ab815df5c2`, later than the required
  `a02b29fb`. From `StakeHub/native/pftl-transaction-inspect`,
  `cargo check --locked` passed. The paths in
  `native/pftl-transaction-inspect/Cargo.toml:8` and `:9` therefore resolve
  and compile against current L1 `main`.
- **PR-added Python tests: pass.** An isolated CPython 3.12.14 environment
  collected 367 added cases: **363 passed and 4 skipped** in 24.47 seconds.
  The skips are the explicit native-binary opt-ins in
  `tests/test_shielded_exit_native_conformance.py:15`.
- **Changed-test-file sweep: qualified pass.** Running all 38 changed test files
  produced **490 passed, 4 skipped, 1 failed**. The sole failure is a
  pre-existing Playwright test, not a PR-added test: after installing Playwright
  and its Chromium bundle, Chromium could not start because this host lacks
  `libnspr4.so`. The test is at
  `tests/test_dashboard_server.py:671` in the reviewed head and already exists
  on the base; the PR only inserts two earlier tests in that file. This is a reviewer-host limitation, not a regression attributed
  to PR #8.
- **No live test networking.** `strace -f -e connect` recorded 13 Internet
  socket connections for the PR-added tests, all to `127.0.0.1` test servers.
  No test contacted an external or live endpoint.
- **StakeHub docs: pass.** `mkdocs build --strict` passed with output directed
  outside the checkout.
- **Archive integrity: pass for the completed-run inventory.** All 190 listed
  files exist and match their SHA-256 digests. `git diff --check` also passed.

The committed validation log at
`docs/handoffs/navcoin-recovery-20260907/validation/stakehub-tests.log:18`
reports 469 passed and 4 skipped but records no invocation or source revision.
It is historical evidence, not a reproducible command for the reviewed head.

## Findings

1. **P1 — A policy denial falls through to an unrestricted direct signer.**

   `stakehub/cli.py:1516` says the agent enforces the destination whitelist
   and USD caps, but `stakehub/cli.py:1543` treats every non-success response,
   explicitly including `policy_denied`, as a reason to request the vault
   passphrase. Lines 1547–1565 then call the direct EVM send, bridge, or
   Hyperliquid deposit implementation, which does not apply the rejected agent
   policy. An operator who supplies the passphrase can therefore complete the
   same transfer after the agent rejects its destination or cap. A policy
   rejection must fail closed; any intended daemon-absent recovery path must be
   distinguished from a policy decision and must not silently discard the
   persisted controls. No test covers this fallback.

2. **P1 — New operational scripts are live merely by being invoked.**

   `scripts/shielded_exit_deploy.py:24` opens a launch session and deploys on
   mainnet or Arbitrum at line 43 without a `--live` flag or typed
   confirmation. `scripts/shielded_exit_probe_arbitrum.py:96` can deploy,
   fund, sign, and send at lines 126, 143, 156, and 181.
   `scripts/shielded_exit_probe_lighter.py:74` wraps and transfers ETH and
   submits the sponsored transaction at lines 92, 97, and 117. The new
   `scripts/pfeth_*.py` and `scripts/pftl_fleet_release_apply.py` scripts
   likewise enter live signing, governance, finality, or fleet mutation paths
   with no dry-run gate. Names and docstrings warning that a probe is live are
   not an execution interlock. Require an explicit live flag and intent-bound
   confirmation after displaying chain, endpoint, addresses, amounts, and
   release identity.

3. **P1 — Failure paths can strand private wallet and note material on validators.**

   `scripts/pfeth_shield.py:81` copies the owner's wallet key to the elected
   validator and installs it at line 85. If the proposer changes, lines 105–108
   exit before the cleanup at line 124; any earlier SSH, parsing, or round
   exception has the same problem. `scripts/pfeth_bridge_out.py:67` similarly
   copies `p_i.key.json` and retains the installed owner key until line 121,
   after signing and submission. `scripts/pfeth_private_egress.py:138` sends a
   private note opening and does not clean it until line 165. None uses a
   `finally` cleanup. This contradicts the local-key boundary stated in
   `stakehub/shielded_exit_runtime.py:1` and can leave spend authority or note
   material on a remote validator after an ordinary error.

4. **P2 — The private-egress script can record and report false success.**

   After its remote certified-round process, `scripts/pfeth_private_egress.py:172`
   sets `round_ok` solely because the fleet tip reached the expected height.
   It does not bind that height to the submitted batch, certificate, or receipt;
   an unrelated block can satisfy the check and cause a later run to skip the
   egress. Lines 178–183 then return success if *any* asset has the requested
   numeric balance, rather than requiring the expected `asset_id`. Verify the
   exact accepted receipt/batch and the exact asset identifier.

5. **P2 — The NAVCoin deposit path hardcodes the live Ethereum RPC.**

   `stakehub/navcoin_deposit.py:25` fixes the endpoint to
   `https://ethereum-rpc.publicnode.com`; the parser at lines 169–175 exposes
   no Ethereum RPC override. The endpoint is used both for preflight at line 210
   and for the agent's approval/deposit submissions at lines 338 and 370. This
   prevents the “configured endpoints” setup described by the machine handoff,
   leaks transaction intent to a fixed third party, and makes the only live
   deposit path depend on one public service. Require an explicit or
   policy-pinned Ethereum RPC and bind its chain/genesis observations into the
   retained intent.

6. **P2 — PR #8 adds an untested, irreversible master-key signing operation.**

   `stakehub/agentd.py:1607` adds the raw
   `hl_register_referrer` agent operation. Any accepted local agent request can
   choose an alphanumeric code and optional base URL, then lines 1617–1622 sign
   and post the one-time Hyperliquid account mutation with the master EVM key.
   It has no CLI confirmation, approved-code binding, launch session, or
   dedicated test, and no caller in the PR. Remove this unrelated signing
   surface or add an explicit, tested authorization ceremony.

7. **P2 — The checked-out evidence is not fully independent of the unmerged L1 handoff.**

   The top-level
   `docs/handoffs/navcoin-recovery-20260907/archive-manifest.json:2` lists 50
   items. Thirty-four verify in this StakeHub PR, but sixteen
   `deployments/a666-source-route-20260907/**` items are absent from both the
   PR and current L1 `main`; they live in the separate L1 handoff/PR #39. The
   completed-run 190-file inventory is self-consistent, but the root historical
   source/deployment manifest is not verifiable from the two target branches
   that the operator described as independently reviewable. Either include a
   clearly self-contained manifest for StakeHub #8 or label the sixteen entries
   as external PR #39 dependencies.

8. **P2 — The “recovery archive is not in Git” handoff claim is false for this PR.**

   The consolidation handoff at
   `docs/handoffs/2026-09-07___dravlic__pr38_merged_arc_deck_unl_proposal.md:207`
   says `docs/handoffs/navcoin-recovery-20260907/` and its ELFs/witness are not
   in Git. PR #8 adds that directory, including both ELF files, witnesses, 190
   completed-run artifacts, and historical scripts. The newer machine handoff
   does describe an archive in the private StakeHub companion repository, so
   the two handoffs disagree. The archive secret scan was clean, but the
   publication and retention decision must be reconciled before relying on
   either statement.

9. **P3 — Exact integer withdrawal amounts are converted through binary floats before signing.**

   `stakehub/shielded_exit_cli.py:220` deliberately parses an exact integer
   amount, but `stakehub/shielded_exit_venues.py:287` passes
   `amount_atoms / 1e9` to Lighter and line 323 passes
   `amount_6dp / 10**6` to Hyperliquid. Values beyond binary64's exact integer
   range can be rounded or truncated by the SDK, so the signed venue amount can
   differ from the displayed and journaled integer. The tests at
   `tests/test_shielded_exit_venues.py:159` and `:221` cover only 1 ETH and
   25 USDC. Enforce a safe exact range and verify the SDK's final serialized
   amount, or use an exact decimal/string interface.

## Areas with no findings

- **Committed credentials and keys:** no seed, mnemonic, private key, API token,
  password, wallet state, SSH credential, or recovery phrase was found in the
  PR, fixtures, ELF strings, binary proof artifacts, or archive. The apparent
  secret-scanner hits are public program verification keys and token/contract
  addresses. Test literals such as `correct-horse` are obvious fixtures.
- **Rejected epoch-7 funding:** the rejected verifier
  `0x5090a2fca72df9a43fa60ead9f6c87a31cead030` and vault
  `0x45cdea31d9b1bc6542ab2a858db624ae087ecd3a` occur only in the quarantined
  recovery evidence. `epoch7/DO-NOT-FUND.txt:1` and the archive README warn
  not to fund them. No current StakeHub module or live funding path references
  either address; the retained `.py.txt` deployment source deploys contracts
  but contains no funding action.
- **NAV-specific defaults:** `wallet nav-roundtrip`,
  `navcoin_deposit.py`, and `navcoin_checkpoint.py` require explicit
  `--execute`. The integrated shielded-exit CLI uses false-by-default
  `--live` or `--broadcast` flags and typed confirmations. The exception is
  the separate direct-script surface in finding 2.
- **Native inspector behavior:** it is read-only, bounds input to 1 MiB,
  validates transaction structure, derives the native transaction ID, and
  explicitly reports that it does not verify signatures or execution.
- **Test network use:** no PR-added test contacted a live endpoint.

## Claims versus diff

| Handoff claim | Review result |
| --- | --- |
| StakeHub #8 has no merge dependency and can be reviewed on its own. | True as a merge-order statement. The native inspector compiles against current L1 `main`. The historical root archive manifest still has the sixteen external PR #39 entries described in finding 7. |
| The PR carries the native runner, governed reserves, CLI/dashboard, bridge/funding modules, tests, proofs, and receipts. | Supported by the diff. The native CLI covers only the native cycle, while the archive records the completed external route. |
| The 10 USDC external round trip completed with 9.932860 USDC returned at PFTL 1020. | Supported as retained evidence: the 190-file inventory is hash-complete, the acceptance and six-validator conservation summaries agree, and this review found no internal amount/height contradiction. Live chains were not re-queried, so this is archive validation rather than a fresh operational attestation. |
| Private Hyperliquid/Lighter work is paused and no success is claimed. | The PR does not claim completion of the privacy objective. “Paused” is documentary rather than enforced: direct live scripts remain immediately executable, as described in finding 2. |
| Generated site/egg output, wallet/key material, and credentials are excluded. | Supported for the PR diff and committed-secret scan. Runtime remote-key handling still has finding 3. |
| The recovery archive is not in Git. | Not supported; finding 8 names the exact contradiction. |
| Epoch-7 pfUSDC contracts must never be funded. | Supported by the active code and explicit quarantine warnings; no funding reference was found. |
