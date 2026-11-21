# Second Set of Eyes: NAVCoin review checklist for agents

Use this to brief review agents. Give an agent one item, or one group, plus the rules and the locations below. Tick the box in the last column when an item has a verdict.

Public version with diagrams: https://postfiat.org/research/schwab-navcoin-handoff/

## Where the code is (main machine)

- Network software: `/home/postfiat/repos/postfiatl1v2-b24-tee-nav`, branch `b23/tee-phase6-mainnet-20261114`, commit `43ddc7d6`. Paths starting `crates/`, `programs/`, `tools/` and `ops/` are here.
- Schwab reader and enclave: `/home/postfiat/repos/navstrategies-b23-tee-nav`, branch `b23/tee-nav-phase1-20261113`. Paths starting `containers/` and `navstrategies/` are here.
- Task Node wallet and coin pages: `/home/postfiat/repos/tasknode-b23-integration`, branch `b23/integration`, commit `483dd32`. Paths starting `server/l1/` and `src/features/wallet/l1/` are here.
- Workers' records and raw evidence: `/home/postfiat/repos/b23-coord/status/` and `/home/postfiat/repos/b23-coord/evidence/`.
- Coin page data, no sign-in: `https://postfiatfoundationv2.taila4ec45.ts.net:8868/api/l1/navcoin/profiles/b24` (also `a666`, `bmnrc`, `b23`).

## What to send back

For each item: the ID, the verdict (PASS, FAIL or UNCLEAR), the evidence (file and line, or the command and its output), and for a FAIL the smallest example that shows it.

## Rules for every agent

- **Read only.** Work on a local copy or a private fork of Ethereum. No transactions on the live network, no funds moved, no services restarted.
- **No secrets.** Never open, print or copy key files, tokens or credentials. Report a secret you find by its path, not its contents.
- **Evidence, not opinion.** Each finding cites a file and line or pastes the command and its output.
- **One verdict per item:** PASS, FAIL or UNCLEAR, followed by what would settle an UNCLEAR.
- **Do not fix.** Report. Fixes are separate work.

### A. Can money be created, taken or stuck?

| ☐ | ID | Throw an agent at | Passes when | Start at |
| --- | --- | --- | --- | --- |
| ☐ | A1 | Crediting the same Ethereum deposit twice | Every replay route is rejected, with a test that shows it | `nav_vault_asset_execution.rs:1232`, `:1954` |
| ☐ | A2 | Crediting a deposit that never finalised | A proof built on a swapped or stale signer set is rejected, and that test exists and runs | `programs/pfusdc-eth-mainnet-ingress/src/lib.rs:336` |
| ☐ | A3 | Every way USDC can leave the vault | The only exit is a valid proof; the admin key can pause and nothing else | `ERC20BridgeVaultL1.sol:127`, `:191` |
| ☐ | A4 | Why USDC deposited now cannot be withdrawn | The claim that the vault's checker pins the older bridge setting is confirmed or disproved from the deployed bytecode | Vault `0xE7A432a2…`, `ERC20BridgeVaultL1.sol:191` |
| ☐ | A5 | Minting past the supply cap or issue capacity | No sequence of creates, including concurrent ones, exceeds either limit | `nav_vault_asset_execution.rs:4729`, `:4070`, `:4577` |
| ☐ | A6 | Rounding at creation and cash-in | Every rounding goes in the protocol's favour and dust cannot be farmed with many small orders | `nav_vault_asset_execution.rs:4482`, `redeem/model.js:10` |
| ☐ | A7 | Cashing in more than the reserve holds | Refused cleanly; the fee reaches the management wallet in the same transaction every time | `nav_vault_asset_execution.rs:5364` |
| ☐ | A8 | Minting wB24 twice from one receipt | A consumed receipt cannot be reused; wrapped supply never exceeds native coins locked | `PFTLUniswapHandoffController.sol:1615`, `:1753` |
| ☐ | A9 | Who can unpause, re-point or upgrade the Ethereum contracts | A written list of every privileged function and the address that holds it, with nothing unexpected | `PFTLUniswapHandoffController.sol`, `PFTLReceiptFinalityVerifierV2.sol:229` |

### B. Is the Schwab number what it claims to be?

| ☐ | ID | Throw an agent at | Passes when | Start at |
| --- | --- | --- | --- | --- |
| ☐ | B1 | Whether the proof pins the exact enclave image | A reading from any other image fails verification, and the pinned image can be rebuilt from source to the same measurement | `yolo_nitro.rs:164`, `b23_nav.rs:141`, `containers/yolo-nitro/` |
| ☐ | B2 | Replaying an old attestation as today's reading | Yesterday's signed result cannot be accepted as a new reading | `b23_nav.rs:141`, `nav_vault_asset_execution.rs:3000` |
| ☐ | B3 | What the machine's operator can change undetected | A written list of everything outside the enclave that influences the result, each shown harmless or flagged | `containers/yolo-nitro/aws/`, `nav_reading.py` |
| ☐ | B4 | Who can change a coin's proof setting | Replacing the pinned program key needs the authority the design says it needs, and no weaker path exists | `nav_sp1_verifier.rs:86`, proof profile registration |
| ☐ | B5 | Recomputing B24's three readings independently | NAV, supply and the split are reproduced from the published values by a script that shares no code with ours | `account_owned_asset_types.rs:813`, coin page data |
| ☐ | B6 | Claiming more Proven or Attested value than the proof supports | The parts can never sum past the total, on chain or on screen | `gallery/model.js:10`, `nav_reserve_public_values.rs:51` |
| ☐ | B7 | What a stale coin actually refuses | BMNRC is past its freshness limit today: confirm by read-only calls that creation and cash-in are refused | Route state for BMNRC |
| ☐ | B8 | The four items that can post to Schwab after the reading | Their worst-case size is estimated against the account and compared with the 0.05 percent cash-in margin | `nav_reading.py` |

### C. Can the Task Node leak or mislead?

| ☐ | ID | Throw an agent at | Passes when | Start at |
| --- | --- | --- | --- | --- |
| ☐ | C1 | Smuggling key material past the relay | Nested, encoded and renamed secrets are all rejected | `custody-boundary.js`, `server/l1/gateway.js:54` |
| ☐ | C2 | Reaching the network signed out or from another site | Both the request path and the live-update path refuse | `server/l1/gateway.js:75`, `:82`, `:101` |
| ☐ | C3 | Calling anything outside the allowed list | Only the listed read and submit calls pass | `lib/rpc-methods.js`, `server/l1/gateway.js:49` |
| ☐ | C4 | The browser vault | The seed is never stored or sent unencrypted, key stretching is adequate, and one account cannot open another's wallet | `lib/vault.js:137`, `:208` |
| ☐ | C5 | Every number on the coin pages | An independent script reads all six validators and Ethereum and matches the four coin pages exactly | `server/l1/navcoin-profile.js:66`, `server/l1/profile/ethereum.js` |
| ☐ | C6 | What the screen says when something fails | No path shows success without an accepted receipt | `market/index.jsx`, `redeem/index.jsx`, `bridge/controller.js` |
| ☐ | C7 | Merging the wallet branch into the public Task Node | A dry-run merge with the full test suite, and a list of what breaks | Task Node `b23/integration` |

### D. Will it keep running?

| ☐ | ID | Throw an agent at | Passes when | Start at |
| --- | --- | --- | --- | --- |
| ☐ | D1 | How the daily job fails | Each failure (expired Schwab access, cloud outage, proof failure, spending cap) ends in a clear state and somebody is told | `ops/b24-live/daily.py` |
| ☐ | D2 | Running the daily job twice in one day | The second run cannot post a conflicting reading | `ops/b24-live/daily.py` |
| ☐ | D3 | Total cloud spend against the $100 cap | A dated total, the run rate, and the day the cap is reached | Cloud billing, job records |
| ☐ | D4 | The network bug that needed manual repair three times at launch | Reproduced on a local copy with a root cause | Launch records, `status/tee.md` |
| ☐ | D5 | Why the fifth validator cannot sign Ethereum-side approvals | Root cause and the steps to restore a spare signer | Validator authority keys (paths only) |
| ☐ | D6 | Merging the network branches into the main line | A merge order, the conflicts, and a build that still produces the live network's state | `b23/*` branches |
| ☐ | D7 | Secrets in the branches and evidence folders before anything goes public | No key, token or credential is committed; anything found is reported by path only | All `b23/*` branches, the evidence folders |

### E. The other coins and the private path

| ☐ | ID | Throw an agent at | Passes when | Start at |
| --- | --- | --- | --- | --- |
| ☐ | E1 | Why A666's pool trades at $0.66 against a NAV of $1.167 | The swap history explains it: never re-centred, sold into, or something else | Pool `0xc5f1e4b5…`, Ethereum logs |
| ☐ | E2 | Moving A666 to a proof setting that records its split | A plan showing its page would then report the Proven and Attested shares, with no change to its supply | `stakehub-six-leg-reserves-v3` profile |
| ☐ | E3 | Restarting BMNRC's reading | The reason it stopped in October is confirmed and the restart steps are listed | BMNRC reading pipeline |
| ☐ | E4 | Private swaps conserving each asset | No shielded swap can create or destroy value in any asset; a spent note cannot be spent again | `orchard_state_application.rs:961`, `:1131` |
| ☐ | E5 | Private create and cash-in using an old price | A private order tied to a stale or superseded reading is refused | `nav_vault_asset_execution.rs:4950`, `:5151` |

### F. Does the writing match the code?

| ☐ | ID | Throw an agent at | Passes when | Start at |
| --- | --- | --- | --- | --- |
| ☐ | F1 | Every file and line on this page | Each reference opens to code that does what the page says it does | The code tables on this page |
| ☐ | F2 | Every number on this page | Each figure is re-read from the network or Ethereum and matches, or is marked out of date | Live numbers and split diagram |
| ☐ | F3 | The unpublished Schwab blog post | A list of every statement that is wrong now that B24 replaced B23 | Site pull request 29 |
| ☐ | F4 | The trust table | For each row, an attempt to name something else the user must trust that the table leaves out | "What you trust, step by step" |

