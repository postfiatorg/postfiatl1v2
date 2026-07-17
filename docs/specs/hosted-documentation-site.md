# Hosted Documentation Site Spec

Status: implemented initial self-hosted site
Date: 2026-05-19
Owner: PostFiat L1

## Objective

Build a hosted documentation site that lets a serious engineer, validator
operator, wallet engineer, RPC integrator, or institutional reviewer understand
PostFiat L1 from first principles on day one.

The site should explain:

- what PostFiat is;
- why the chain exists;
- what has been built;
- how the protocol works;
- how Cobalt governance works;
- how Orchard/Halo2 privacy works;
- how post-quantum authorization works;
- how RPC and the Python client work;
- how validators are launched and operated;
- how to inspect evidence instead of trusting prose.

The standard is engineering-grade product documentation: clear navigation,
clean URLs, searchable pages, code references, command examples, diagrams,
evidence anchors, and explicit ownership of what is current.

## Hosting Decision

Use a static documentation site.

Preferred first implementation:

- generator: MkDocs with Material theme;
- docs source: curated subset of the unified `docs/` tree;
- build config: `mkdocs.yml`;
- deploy target: self-hosted static files on a project-controlled machine;
- initial URL shape: `http://<docs-host-ip>:8088/`;
- custom URL when DNS is available: `https://docs.postfiat.org/`.

Why this choice:

- the repo already uses Markdown heavily;
- Python tooling already exists in the repo;
- static output is easy to audit and host;
- `mkdocs build --strict` catches broken links and bad navigation;
- Material gives search, navigation, code highlighting, tabs, admonitions, and
  good mobile rendering without adding a React app;
- a self-hosted service gives a real URL without tying the docs to an external
  organization namespace.

The first hosted version should run on an ordinary machine over a single TCP
port. The machine owner can then open that port with UFW when ready. The docs
implementation should not silently change firewall policy.

Docusaurus is a reasonable later option if the docs need React components,
multi-version API docs, or large interactive examples. It is not necessary for
the first hosted version.

## Source Layout

Do not publish every file under `docs/` as the hosted site.

The repo's `docs/` tree contains hosted pages, status logs, archived agent work,
research prompts, internal handoffs, and evidence breadcrumbs. Those files share
one source tree, but only the curated subset configured in `mkdocs.yml` should be
published.

Use this curated public docs layout inside `docs/`:

```text
mkdocs.yml
docs/
  index.md
  whitepaper.md
  start/
    first-day.md
    glossary.md
    repo-map.md
  architecture/
    overview.md
    transaction-lifecycle.md
    state-and-storage.md
    finality.md
    evidence-model.md
  governance/
    cobalt.md
    cobalt-implementation.md
    cobalt-adversarial-testing.md
    validator-registry.md
  privacy/
    overview.md
    orchard-halo2.md
    deposit-spend-withdraw.md
    disclosure.md
    rpc-and-resource-policy.md
  quantum/
    authorization.md
    signature-size-and-certificates.md
    wallet-implications.md
  rpc/
    overview.md
    methods.md
    account-history.md
    write-policy.md
    examples.md
  python/
    quickstart.md
    client-api.md
    examples.md
  wallets/
    transparent-wallet.md
    shielded-wallet.md
    custody-and-exchange.md
  validators/
    overview.md
    launch.md
    operations.md
    history-retention.md
    monitors.md
    emergency-key-rotation.md
  evidence/
    index.md
    controlled-testnet.md
    cobalt.md
    privacy.md
    latency.md
    rpc-and-wallets.md
  security/
    threat-model.md
    redaction-policy.md
    public-launch-boundary.md
```

The hosted site should link back to source files in `docs/`, `crates/`,
`scripts/`, `python/`, and `reports/`, but it should not dump every raw file
into the nav.

## Navigation

Top-level navigation:

1. Start Here
2. Whitepaper
3. Architecture
4. Cobalt Governance
5. Privacy
6. Quantum Authorization
7. RPC
8. Python Client
9. Wallets
10. Validators
11. Evidence
12. Security And Launch Boundary

The first page should answer the basic questions without making the reader hunt:

- PostFiat is an XRP-style authority-validator L1.
- It is written in Rust.
- It uses post-quantum account and validator authorization.
- It has fast certified finality evidence.
- It has full Cobalt controlled-testnet mechanics.
- It has Orchard/Halo2 privacy-alpha flows.
- It has fixed supply, fee burn, and no native validator reward schedule.
- It has RPC, Python client, wallet, validator, monitor, and evidence tooling.

## Page Specs

### Home

Purpose: make the project legible in five minutes.

Content:

- one-paragraph thesis;
- "What exists now" capability table;
- "How to inspect it" command/evidence links;
- links to whitepaper, Cobalt, privacy, RPC, Python, validators;
- current controlled-testnet status summary;
- exact repo revision used to generate the site.

### Whitepaper

Purpose: canonical project paper.

Source:

- [whitepaper.md](../whitepaper.md)

Implementation:

- copy or generate the hosted page from `docs/whitepaper.md`;
- keep one canonical source of truth;
- add page-local links into architecture, Cobalt, privacy, RPC, and evidence
  pages.

### First-Day Engineer Guide

Purpose: new engineer onboarding.

Content:

- repo map;
- crate map;
- how to build;
- how to run local devnet;
- how to send a transparent transfer;
- how to query RPC;
- how to run the core smoke checks;
- where not to put private material;
- what docs are current and what docs are historical.

Sources:

- [README.md](../../README.md)
- `Cargo.toml`
- `crates/`
- `scripts/devnet-up`
- `scripts/devnet-submit-transfer`
- `scripts/devnet-sdk-rpc-smoke`

### Architecture Overview

Purpose: explain the chain as a system.

Content:

- component diagram;
- transaction lifecycle diagram;
- state transition model;
- block/certificate/receipt relationship;
- storage and snapshot model;
- retained-history model;
- how governance and ordering are separated.

Sources:

- `crates/types/src/lib.rs`
- `crates/execution/src/lib.rs`
- `crates/node/src/lib.rs`
- `crates/storage/src/lib.rs`
- `crates/ordering_fast/src/lib.rs`
- `docs/status/controlled-testnet-burndown.md`

### Finality

Purpose: explain how transactions clear.

Content:

- proposer, vote, certificate, commit, receipt;
- `tx` finality lookup;
- current latency evidence;
- why the old harness was slow;
- current target and what evidence supports it.

Sources:

- `crates/node/src/block_finality.rs`
- `scripts/testnet-tx-finality-latency-benchmark`
- `reports/testnet-tx-finality-latency-benchmark/local-gate-20round-20260514T142731Z/testnet-tx-finality-latency-benchmark.json`
- `reports/testnet-remote-ssh-smoke/optimized-latency-20260514T143534Z/testnet-remote-ssh-smoke.json`

### Cobalt Governance

Purpose: make Cobalt understandable without handwaving.

Pages:

- plain-English Cobalt;
- Cobalt vs XRPL UNL;
- trust views and essential subsets;
- linkedness and unsafe graph rejection;
- RBC, ABBA, MVBA, and DABC;
- validator registry and trust-graph transitions;
- adversarial testing matrix;
- controlled readiness vs public topology evidence.

Sources:

- `docs/references/cobalt-bft-governance-in-open-networks.md`
- `docs/governance/full-cobalt-shipping-plan.md`
- `docs/status/full-cobalt-burndown.md`
- `docs/status/cobalt-adversarial-burndown.md`
- `crates/consensus_cobalt/src/lib.rs`
- `crates/consensus_cobalt/examples/`
- `scripts/testnet-cobalt-controlled-readiness-gate`
- `reports/testnet-cobalt-controlled-readiness-gate/amendment-replay-contract-clean-v0-20260519T145213Z/testnet-cobalt-controlled-readiness-gate.json`

### Privacy

Purpose: document the Orchard/Halo2 path as an engineering system.

Pages:

- privacy overview;
- Orchard/Halo2 adapter;
- transparent-to-Orchard deposit;
- Orchard spend;
- Orchard withdraw;
- scanning and viewing keys;
- selective disclosure;
- pool report and anonymity-bound telemetry;
- RPC resource policy;
- production-hardening roadmap.

Content rules:

- say what exists;
- explain controlled privacy-alpha boundaries in one place;
- do not bury the working deposit/spend/withdraw flow under caveats;
- do not call debug proof paths production privacy;
- distinguish Orchard/Halo2 proof verification from future post-quantum
  note-encryption and proof-system migration.

Sources:

- `docs/status/privacy-production-burndown.md`
- `docs/status/orchard-halo2-implementation-plan.md`
- `crates/privacy/src/lib.rs`
- `crates/privacy_orchard/src/lib.rs`
- `crates/privacy_orchard/src/types.rs`
- `crates/privacy_orchard/src/verify.rs`
- `crates/node/src/privacy.rs`
- `scripts/testnet-orchard-wallet-finality-smoke`
- `scripts/testnet-live-orchard-full-flow-smoke`
- `reports/testnet-live-orchard-full-flow/live-orchard-full-flow-20260515T183724Z/testnet-live-orchard-full-flow.json`
- `reports/testnet-orchard-privacy-audit-packet/orchard-privacy-audit-packet-20260515T185212Z/orchard-privacy-audit-packet.json`

### Quantum Authorization

Purpose: explain what "post-quantum" means in this chain.

Content:

- ML-DSA-style account signatures;
- validator signatures and transport envelopes;
- certificate-size implications;
- wallet and custodian implications;
- what is transparent-PQ today;
- what remains for end-to-end post-quantum shielded value.

Sources:

- `crates/crypto_provider/src/lib.rs`
- `crates/types/src/lib.rs`
- `crates/node/src/transport_cli.rs`
- `scripts/testnet-ml-dsa-performance-smoke`
- `docs/specs/account-key-rotation-boundary.md`
- `docs/specs/transparent-transaction-envelope.md`

### RPC

Purpose: make integration possible without reading node internals.

Pages:

- RPC overview;
- method inventory;
- read methods;
- controlled write policy;
- `tx` finality;
- `account_tx` and retained history;
- Orchard pool report;
- JSON examples;
- error model;
- rate limits and public-edge posture.

Sources:

- `docs/runbooks/rpc-method-inventory.md`
- `docs/runbooks/public-rpc-operator-policy.md`
- `docs/runbooks/account-tx-index.md`
- `docs/runbooks/controlled-write-edge-policy.md`
- `crates/rpc_sdk/src/lib.rs`
- `crates/node/src/rpc_cli.rs`
- `scripts/testnet-rpc-doctor`
- `scripts/postfiat-rpc-account-tx`

### Python Client

Purpose: give integrators a practical client.

Pages:

- install and import;
- quickstart;
- status/ledger/fee examples;
- transaction finality examples;
- account history examples;
- CSV export;
- error handling;
- relation to raw RPC.

Sources:

- `python/postfiat_rpc/client.py`
- `python/postfiat_rpc/__main__.py`
- `docs/runbooks/python-rpc-client.md`
- `reports/testnet-live-python-rpc-client-smoke/`
- `reports/testnet-six-wallet-account-tx-smoke/`

### Wallets

Purpose: document wallet behavior and custody implications.

Content:

- transparent wallet key generation;
- signing and fee quote;
- SDK wallet finality;
- account history;
- Orchard wallet commands;
- scan/disclose/spend/withdraw flows;
- exchange/custody model;
- key-rotation boundary.

Sources:

- `docs/specs/wallet-exchange-custody-model.md`
- `docs/specs/account-key-rotation-boundary.md`
- `docs/runbooks/sdk-wallet-flow.md`
- `scripts/testnet-live-wallet-finality`
- `scripts/testnet-wallet-test-vectors-smoke`

### Validators

Purpose: make operators competent.

Pages:

- validator overview;
- launch package flow;
- service layout;
- history retention;
- partial-history validation;
- validator doctor;
- monitor snapshot;
- restart/outage drills;
- emergency key rotation;
- placement manifest and public-topology evidence.

Sources:

- `docs/runbooks/controlled-testnet-operator-launch.md`
- `docs/runbooks/operator-day-two.md`
- `docs/runbooks/validator-history-retention.md`
- `docs/runbooks/validator-doctor.md`
- `docs/runbooks/validator-emergency-key-rotation.md`
- `scripts/testnet-validator-doctor-smoke`
- `scripts/testnet-monitor-snapshot-smoke`
- `docs/examples/controlled-testnet-placement-manifest.example.json`

### Evidence

Purpose: let reviewers verify claims.

Content:

- curated evidence index;
- report type glossary;
- controlled-testnet evidence;
- Cobalt evidence;
- privacy evidence;
- latency evidence;
- RPC/wallet evidence;
- redaction policy;
- commands to regenerate selected packets.

Implementation:

- do not publish private-material directories;
- link only to redaction-safe reports;
- every evidence page should include report path, date, revision when available,
  what it proves, and what command generated it.

Sources:

- `reports/testnet-controlled-launch-evidence-pack/`
- `reports/testnet-cobalt-controlled-readiness-gate/`
- `reports/testnet-orchard-privacy-audit-packet/`
- `reports/testnet-tx-finality-latency-benchmark/`
- `reports/testnet-rpc-doctor/`
- `reports/testnet-validator-doctor/`
- `reports/testnet-monitor-snapshot/`

## Claim And Redaction Policy

The docs site must never publish:

- private keys;
- mnemonic material;
- validator-private launch material;
- `reports/testnet-private-key-material/`;
- live credentials;
- raw SSH inventories;
- private Orchard witness material such as spending keys, full viewing keys,
  note seeds, or Merkle auth paths;
- unredacted machine or operator details unless explicitly intended for public
  topology evidence.

Every public-facing capability claim needs one of:

- code reference;
- test reference;
- script reference;
- redaction-safe report reference;
- explicit roadmap label.

Controlled-testnet facts and public-launch requirements should be separated.
The docs should not block engineering reality on outside operators that are not
part of the controlled-testnet phase, and should not present controlled
infrastructure as public decentralization.

## Build And Deploy

Add:

```text
mkdocs.yml
docs/
requirements-docs.txt
scripts/docs-site-redaction-check
scripts/docs-site-evidence-index
scripts/docs-site-build
scripts/docs-site-serve
systemd/postfiat-docs.service.example
```

Local build:

```bash
python3 -m pip install -r requirements-docs.txt
mkdocs build --strict
mkdocs serve -a 127.0.0.1:8000
```

Self-hosted build and serve:

```bash
scripts/docs-site-build
POSTFIAT_DOCS_USER=postfiat POSTFIAT_DOCS_PASSWORD=<generated-password> \
  scripts/docs-site-serve --host 0.0.0.0 --port 8088
```

The resulting URL is:

```text
http://<docs-host-ip>:8088/
```

The browser should prompt for HTTP Basic Auth credentials. This is a lightweight
access gate for the temporary direct-IP docs host. For long-term public use,
place nginx or Caddy with TLS in front of the static site.

Firewall step for the machine owner:

```bash
sudo ufw allow 8088/tcp
```

Do not bake the UFW command into the docs server script. Serving the docs and
opening the firewall are separate operator actions.

CI:

1. install docs requirements;
2. run redaction scan over `docs/` and generated evidence index;
3. run `mkdocs build --strict`;
4. upload the static site artifact.

Build workflow:

```yaml
name: docs
on:
  push:
    branches: [main]
permissions:
  contents: read
jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with:
          python-version: "3.12"
      - run: python -m pip install -r requirements-docs.txt
      - run: scripts/docs-site-redaction-check
      - run: mkdocs build --strict
      - uses: actions/upload-artifact@v4
        with:
          name: postfiat-docs-site
          path: site
```

Deployment is a machine operation:

1. build the static site into `site/`;
2. copy `site/` to the docs host;
3. serve it with `scripts/docs-site-serve`, nginx, or Caddy;
4. expose the chosen TCP port with UFW when the owner is ready;
5. point DNS at the host if a custom domain is desired.

## Automation

The site should have two generated pieces.

### Evidence Index

`scripts/docs-site-evidence-index` should read a curated allowlist of report
paths and generate `docs/evidence/index.md`.

Each entry:

- report path;
- report kind;
- date;
- commit/revision if present;
- pass/fail status;
- one-sentence interpretation;
- source command.

The generator should not crawl all of `reports/` automatically. Evidence should
be curated so stale debug output does not become public documentation.

### RPC Method Inventory

Generate or refresh `docs/rpc/methods.md` from:

- `docs/runbooks/rpc-method-inventory.md`;
- `crates/rpc_sdk/src/lib.rs`;
- `crates/node/src/rpc_cli.rs`.

The generated page should include request/response examples, public-edge
posture, and whether the method is read-only, controlled-write, or local-only.

## Diagrams

Use Mermaid diagrams in Markdown for:

- system architecture;
- transparent transaction lifecycle;
- Cobalt governance transition;
- Orchard deposit/spend/withdraw;
- validator launch and evidence flow;
- RPC read/write boundary.

Avoid decorative diagrams. Every diagram should answer an engineering question.

## Acceptance Criteria

The first hosted version is done when:

- a machine URL such as `http://<docs-host-ip>:8088/` serves the docs;
- `mkdocs build --strict` passes in CI;
- the site has search and stable navigation;
- the home page states what exists now;
- the whitepaper renders from the canonical paper;
- Cobalt, privacy, RPC, Python, wallet, and validator sections all exist;
- every major subsystem has code and evidence anchors;
- the evidence index links only redaction-safe reports;
- private material patterns are scanned before publish;
- first-day engineer guide can take a new engineer from clone to local smoke;
- RPC docs include method inventory and examples;
- Python docs include install/import/query examples;
- validator docs include launch, doctor, monitor, retention, and emergency
  key-rotation paths;
- public-launch boundaries are centralized and not repeated as apology text on
  every page.

## Implementation Slices

### Slice 1: Site Skeleton

- Add `mkdocs.yml`.
- Add `docs/index.md`.
- Add requirements file.
- Add strict local build.
- Add CI build workflow.
- Add self-hosted serve script.
- Add example systemd unit for a persistent docs service.
- Add redaction-check stub with key-field denylist.

Exit: hosted skeleton builds locally and in CI, and can be served at
`http://<docs-host-ip>:8088/` after the operator opens UFW.

### Slice 2: Core Narrative

- Port the canonical whitepaper into the site.
- Add first-day guide.
- Add architecture overview.
- Add repo/crate map.
- Add launch-boundary page.

Exit: a new engineer can understand the chain shape without reading internal
status docs.

### Slice 3: Protocol Sections

- Add Cobalt pages.
- Add finality page.
- Add quantum authorization page.
- Add privacy pages.

Exit: every protocol pillar has plain-English explanation, implementation
details, code refs, and evidence refs.

### Slice 4: Integrator Sections

- Add RPC docs.
- Add Python client docs.
- Add wallet docs.
- Add examples and command snippets.

Exit: an external engineer can query the chain, inspect finality, and pull
account history with the documented tools.

### Slice 5: Operator Sections

- Add validator launch docs.
- Add day-two operations.
- Add history retention.
- Add monitors and doctor tooling.
- Add emergency key rotation.

Exit: a validator operator can launch and inspect a controlled validator from
docs alone.

### Slice 6: Evidence Automation

- Add curated evidence allowlist.
- Add evidence index generator.
- Add RPC inventory generator if useful.
- Enforce redaction scan in docs CI.

Exit: the hosted docs tie claims to reports without publishing unsafe material.

## Initial URL Plan

1. Build the static docs site on the current docs host.
2. Serve it on `0.0.0.0:8088`.
3. Visit `http://<docs-host-ip>:8088/` from a browser.
4. If the port is blocked, the machine owner opens it with
   `sudo ufw allow 8088/tcp`.
5. Add `docs.postfiat.org` by pointing DNS to the docs host and placing nginx
   or Caddy in front of the static site when the domain is ready.

The site should be useful by direct host URL immediately. The custom domain and
TLS are distribution improvements, not implementation blockers.
