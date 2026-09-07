# Validator correlation from frozen identity packets

SHADOW_ONLY. Computed deterministically from the Machine-Readable Summary of each
frozen packet in `validator-identity-packets-20260904` (packet-set SHA-256 `8051f392e60d84a687076dc241ddf722859db7c06718dd12139c3109548523df`).
No model call, no network access. A link means two packets share public-identity
signals; it does not prove common key control.

- validators: 55; pairs with any signal: 106 of 1485; strong pairs: 3; clusters: 1

## Clusters (strong links: same entity, alias, X handle, or registrable domain)

- **Post Fiat / Post Fiat (public project name; legal entity not established)** (postfiat; canonical_entity_match, registrable_domain_match, x_handle_match)
  - `nHUc7VSYA6xvFakSvuTojJQucBNukKwmtguUG2HMT9Xp9dKzkpvJ` — postfiat · postfiat.org
  - `nHUkhbZe9ncdmhn6dbd5x7391ymwCS3YZEMWjysP9fSiDtau9YEe` — postfiat · postfiat.org
  - `nHUso5gdgQnewAsk5QT1aFr897g6YLaL697iyuknmSqd7pbqz5Td` — postfiat · postfiat.org

## Top pairs by strength

| strength | a | b | signals |
| --- | --- | --- | --- |
| 1.00 | Post Fiat (postfiat) | Post Fiat (public project name; legal entity not established) (postfiat) | evidence_host_overlap, registrable_domain_match, x_handle_match |
| 1.00 | Post Fiat (postfiat) | Post Fiat (postfiat) | canonical_entity_match, evidence_host_overlap, registrable_domain_match, x_handle_match |
| 1.00 | Post Fiat (public project name; legal entity not established) (postfiat) | Post Fiat (postfiat) | evidence_host_overlap, registrable_domain_match, x_handle_match |
| 0.22 | Ripple Labs Inc. (xrpl) | Blockdaemon Inc. (xrpl) | incorporation_region_match, operating_region_overlap |
| 0.17 | pft.xbtseal.com (postfiat) | pfthaploid.com (postfiat) | evidence_host_overlap |
| 0.12 | pft.hit0ri.xyz (postfiat) | pft.akirax.xyz (postfiat) | evidence_host_overlap |
| 0.10 | pft.hit0ri.xyz (postfiat) | pft.whiteguy.eu (postfiat) | evidence_host_overlap |
| 0.10 | pft.akirax.xyz (postfiat) | pft.whiteguy.eu (postfiat) | evidence_host_overlap |
| 0.09 | dex3333.github.io (postfiat) | local-maxi.github.io (postfiat) | evidence_host_overlap |
| 0.09 | pft.xbtseal.com (postfiat) | Post Fiat (public project name; legal entity not established) (postfiat) | evidence_host_overlap |
| 0.09 | pft.xbtseal.com (postfiat) | Post Fiat (postfiat) | evidence_host_overlap |
| 0.09 | postfiat.live (postfiat) | local-maxi.github.io (postfiat) | evidence_host_overlap |
| 0.09 | validator.pftperry.com (postfiat) | Post Fiat (public project name; legal entity not established) (postfiat) | evidence_host_overlap |
| 0.09 | validator.pftperry.com (postfiat) | Post Fiat (postfiat) | evidence_host_overlap |
| 0.09 | lc66validator.postfiatcn.org (postfiat) | Post Fiat (public project name; legal entity not established) (postfiat) | evidence_host_overlap |
| 0.09 | lc66validator.postfiatcn.org (postfiat) | Post Fiat (postfiat) | evidence_host_overlap |
| 0.09 | Auri_0x (postfiat) | Post Fiat (public project name; legal entity not established) (postfiat) | evidence_host_overlap |
| 0.09 | Auri_0x (postfiat) | Post Fiat (postfiat) | evidence_host_overlap |
| 0.09 | pfthaploid.com (postfiat) | Post Fiat (public project name; legal entity not established) (postfiat) | evidence_host_overlap |
| 0.09 | pfthaploid.com (postfiat) | Post Fiat (postfiat) | evidence_host_overlap |
| 0.08 | Blockdaemon Inc. (xrpl) | STRALINK INNOVATIONS TECHNOLOGIES LIMITADA (xrpl) | evidence_host_overlap, operating_region_overlap |
| 0.07 | Bithomp (xrpl) | nHBidG3pZK11 (xrpl) | evidence_host_overlap |
| 0.07 | pft.hit0ri.xyz (postfiat) | local-maxi.github.io (postfiat) | evidence_host_overlap |
| 0.07 | pft.hit0ri.xyz (postfiat) | Post Fiat (public project name; legal entity not established) (postfiat) | evidence_host_overlap |
| 0.07 | pft.hit0ri.xyz (postfiat) | Post Fiat (postfiat) | evidence_host_overlap |
| 0.07 | dex3333.github.io (postfiat) | postfiat.live (postfiat) | evidence_host_overlap |
| 0.07 | pft.xbtseal.com (postfiat) | validator.pftperry.com (postfiat) | evidence_host_overlap |
| 0.07 | pft.xbtseal.com (postfiat) | lc66validator.postfiatcn.org (postfiat) | evidence_host_overlap |
| 0.07 | pft.xbtseal.com (postfiat) | Auri_0x (postfiat) | evidence_host_overlap |
| 0.07 | pft.xbtseal.com (postfiat) | Post Fiat (postfiat) | evidence_host_overlap |
| 0.07 | Ripple Labs Inc. (xrpl) | Cabbit Technology LLC (xrpl) | evidence_host_overlap |
| 0.07 | validator.pftperry.com (postfiat) | lc66validator.postfiatcn.org (postfiat) | evidence_host_overlap |
| 0.07 | validator.pftperry.com (postfiat) | Auri_0x (postfiat) | evidence_host_overlap |
| 0.07 | validator.pftperry.com (postfiat) | Post Fiat (postfiat) | evidence_host_overlap |
| 0.07 | validator.pftperry.com (postfiat) | pfthaploid.com (postfiat) | evidence_host_overlap |
| 0.07 | lc66validator.postfiatcn.org (postfiat) | Auri_0x (postfiat) | evidence_host_overlap |
| 0.07 | lc66validator.postfiatcn.org (postfiat) | Post Fiat (postfiat) | evidence_host_overlap |
| 0.07 | lc66validator.postfiatcn.org (postfiat) | pfthaploid.com (postfiat) | evidence_host_overlap |
| 0.07 | Blockdaemon Inc. (xrpl) | Jon Nilsen (xrpl) | evidence_host_overlap |
| 0.07 | XLS Labs Inc. (xrpl) | Post Fiat (public project name; legal entity not established) (postfiat) | evidence_host_overlap |
