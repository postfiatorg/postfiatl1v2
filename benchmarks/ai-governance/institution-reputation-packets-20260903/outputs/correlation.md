# Validator correlation from frozen identity packets

SHADOW_ONLY. Computed deterministically from the Machine-Readable Summary of each
frozen packet in `validator-identity-packets-20260901` (packet-set SHA-256 `b198e232baa644731b38e2f6db3989c798156700ebc67856a193b32bb941d4bd`).
No model call, no network access. A link means two packets share public-identity
signals; it does not prove common key control.

- validators: 55; pairs with any signal: 227 of 1485; strong pairs: 3; clusters: 1

## Clusters (strong links: same entity, alias, X handle, or registrable domain)

- **Post Fiat** (postfiat; canonical_entity_match, registrable_domain_match, x_handle_match)
  - `nHUc7VSYA6xvFakSvuTojJQucBNukKwmtguUG2HMT9Xp9dKzkpvJ` — postfiat · postfiat.org
  - `nHUkhbZe9ncdmhn6dbd5x7391ymwCS3YZEMWjysP9fSiDtau9YEe` — postfiat · postfiat.org
  - `nHUso5gdgQnewAsk5QT1aFr897g6YLaL697iyuknmSqd7pbqz5Td` — postfiat · postfiat.org

## Top pairs by strength

| strength | a | b | signals |
| --- | --- | --- | --- |
| 1.00 | Post Fiat (postfiat) | Post Fiat (postfiat) | canonical_entity_match, registrable_domain_match, x_handle_match |
| 1.00 | Post Fiat (postfiat) | Post Fiat (postfiat) | canonical_entity_match, evidence_host_overlap, registrable_domain_match, x_handle_match |
| 1.00 | Post Fiat (postfiat) | Post Fiat (postfiat) | canonical_entity_match, official_host_overlap, registrable_domain_match, x_handle_match |
| 0.38 | Ripple Labs Inc. (xrpl) | Blockdaemon Inc. (xrpl) | evidence_host_overlap, incorporation_region_match, operating_region_overlap |
| 0.15 | Scrambled Egg Technologies, LLC (xrpl) | XLS Labs Inc. (xrpl) | evidence_host_overlap, operating_region_overlap |
| 0.12 | JollyDinger (postfiat) | validator.pftperry.com (postfiat) | evidence_host_overlap |
| 0.12 | JollyDinger (postfiat) | lc66validator.postfiatcn.org (postfiat) | evidence_host_overlap |
| 0.12 | JollyDinger (postfiat) | Post Fiat (postfiat) | evidence_host_overlap |
| 0.12 | Scrambled Egg Technologies, LLC (xrpl) | Christian Katczynski (xrpl) | evidence_host_overlap |
| 0.12 | validator.pftperry.com (postfiat) | lc66validator.postfiatcn.org (postfiat) | evidence_host_overlap |
| 0.12 | validator.pftperry.com (postfiat) | Post Fiat (postfiat) | evidence_host_overlap |
| 0.12 | lc66validator.postfiatcn.org (postfiat) | Post Fiat (postfiat) | evidence_host_overlap |
| 0.12 | XLS Labs Inc. (xrpl) | Christian Katczynski (xrpl) | evidence_host_overlap |
| 0.12 | pfthaploid.com (postfiat) | Post Fiat (postfiat) | evidence_host_overlap |
| 0.12 | tequ (xrpl) | JollyDinger (postfiat) | evidence_host_overlap |
| 0.12 | tequ (xrpl) | validator.pftperry.com (postfiat) | evidence_host_overlap |
| 0.12 | tequ (xrpl) | lc66validator.postfiatcn.org (postfiat) | evidence_host_overlap |
| 0.12 | tequ (xrpl) | Post Fiat (postfiat) | evidence_host_overlap |
| 0.09 | JollyDinger (postfiat) | pft.xbtseal.com (postfiat) | evidence_host_overlap |
| 0.09 | JollyDinger (postfiat) | pft.akirax.xyz (postfiat) | evidence_host_overlap |
| 0.09 | JollyDinger (postfiat) | Auri (postfiat) | evidence_host_overlap |
| 0.09 | JollyDinger (postfiat) | pft.whiteguy.eu (postfiat) | evidence_host_overlap |
| 0.09 | JollyDinger (postfiat) | Preaware (postfiat) | evidence_host_overlap |
| 0.09 | JollyDinger (postfiat) | pfthaploid.com (postfiat) | evidence_host_overlap |
| 0.09 | Vet (xrpl) | Scrambled Egg Technologies, LLC (xrpl) | evidence_host_overlap |
| 0.09 | Vet (xrpl) | XLS Labs Inc. (xrpl) | evidence_host_overlap |
| 0.09 | Vet (xrpl) | Christian Katczynski (xrpl) | evidence_host_overlap |
| 0.09 | Scrambled Egg Technologies, LLC (xrpl) | XRPK (XRP Kuwait) (xrpl) | evidence_host_overlap |
| 0.09 | pft.xbtseal.com (postfiat) | validator.pftperry.com (postfiat) | evidence_host_overlap |
| 0.09 | pft.xbtseal.com (postfiat) | lc66validator.postfiatcn.org (postfiat) | evidence_host_overlap |
| 0.09 | pft.xbtseal.com (postfiat) | Post Fiat (postfiat) | evidence_host_overlap |
| 0.09 | Ricky Owens (xrpl) | XRPK (XRP Kuwait) (xrpl) | evidence_host_overlap |
| 0.09 | validator.pftperry.com (postfiat) | pft.akirax.xyz (postfiat) | evidence_host_overlap |
| 0.09 | validator.pftperry.com (postfiat) | Auri (postfiat) | evidence_host_overlap |
| 0.09 | validator.pftperry.com (postfiat) | pft.whiteguy.eu (postfiat) | evidence_host_overlap |
| 0.09 | validator.pftperry.com (postfiat) | Preaware (postfiat) | evidence_host_overlap |
| 0.09 | validator.pftperry.com (postfiat) | pfthaploid.com (postfiat) | evidence_host_overlap |
| 0.09 | lc66validator.postfiatcn.org (postfiat) | pft.akirax.xyz (postfiat) | evidence_host_overlap |
| 0.09 | lc66validator.postfiatcn.org (postfiat) | Auri (postfiat) | evidence_host_overlap |
| 0.09 | lc66validator.postfiatcn.org (postfiat) | pft.whiteguy.eu (postfiat) | evidence_host_overlap |
