# Live Testnet UNL Round Evidence - 2026-05-31

Source endpoints:

- Canonical testnet validator list: <https://postfiat.org/testnet_vl.json>
- Public scoring rounds API: <https://scoring-testnet.postfiat.org/api/scoring/rounds?limit=10>
- Public scoring config API: <https://scoring-testnet.postfiat.org/api/scoring/config>

## Current Public Validator List

The canonical public testnet validator list is a signed validator-list payload at
`https://postfiat.org/testnet_vl.json`.

Decoded current VL blob:

| Field | Value |
| --- | --- |
| Publisher public key | `ED3F1E0DA736FCF99BE2880A60DBD470715C0E04DD793FB862236B070571FC09E2` |
| VL format version | `2` |
| Current signed VL sequence | `5` |
| Effective time | `2026-05-26T17:50:23Z` |
| Expiration time | `2027-10-08T17:20:23Z` |
| Validator count | `20` |

The current signed VL sequence `5` matches the latest complete scoring round
reported by the public rounds API.

## Public Round History

The public scoring rounds API reports multiple validator-list publication
rounds. Rounds 4 through 7 completed successfully, each producing a validator
list sequence, final audit bundle CID, GitHub Pages commit, and PFTL memo
transaction hash.

| Round | Status | VL sequence | Completed at | Final bundle CID | GitHub Pages commit | Memo tx hash |
| ---: | --- | ---: | --- | --- | --- | --- |
| 7 | `COMPLETE` | 5 | `2026-05-26T17:20:31.833174Z` | `QmZaR7brX1oEQnfYzunPNnNoSudEmrzo16ZUmnp39Th7LP` | `c5906269b34ccd1742a563257969385104276227` | `12FE5A2787E418D85F9CC832CB7EA19B37EAA20C328D8F0F8CA116E235E4D07F` |
| 6 | `COMPLETE` | 4 | `2026-05-19T17:10:47.660000Z` | `QmbyCftSh2THm5fJAFpoW5QDHJko7kpDgeSH3DEjRHUgda` | `33b1e30a14a76e7bdbc5c96275dcbedda1cae78f` | `CE4F9490CD49136441914B75D0F7F56EB595ADFC078C1251D79AE9F54B1D1E4F` |
| 5 | `COMPLETE` | 3 | `2026-05-12T17:03:27.453750Z` | `QmWkUuk1EntfTa7h2VrE1CXbEB6Ne4fiRrXhZGTBfjAQo9` | `00d927c222348989977e33adb98af315bbc5037a` | `15B9D75CF11E72593F6562DD1383D9F32A30E852A1B1855FFB14AC9D951BCB7E` |
| 4 | `COMPLETE` | 2 | `2026-05-05T16:57:45.204394Z` | `QmPxXnCVvPcFvvPnksqTKXwwH6tBWWTdncf4nWjrAKDk4r` | `940d866df4709b90a8ed5ebdf7c8e907b81d390e` | `0290FA98B6B301B9F1B8AC14DC53BBE906E893AA22757E93EBCD31CD8C2023B0` |
| 3 | `VL_PUBLISHED_MEMO_FAILED` | 1 | `2026-05-05T16:48:43.284837Z` | `QmPZPHd2Az2zgymJQG1o3om3LweQr8TvoZYuf2ew2V6DNF` | `f51f2d9a0ce55bf864ee8d60ad3eb33bf0934be4` | null |

## Cadence Evidence

The public scoring config reports:

| Field | Value |
| --- | ---: |
| `cadence_hours` | `168.0` |
| `unl_score_cutoff` | `40` |
| `unl_max_size` | `20` |
| `unl_min_score_gap` | `5` |

This supports the phrasing that Phase 1 is a live weekly testnet validator-list
scoring and publication process, not merely a design target.

## Whitepaper-Safe Claim

Phase 1 is live. The public testnet has already run multiple validator-list
scoring and publication rounds. The current canonical signed testnet VL is
sequence `5`, contains `20` validators, is effective from
`2026-05-26T17:50:23Z`, and corresponds to public round 7. The rounds API shows
four consecutive fully complete rounds with final audit bundle CIDs, GitHub
Pages publication commits, and PFTL memo transaction hashes.

