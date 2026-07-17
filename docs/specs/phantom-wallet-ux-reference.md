# Phantom Wallet UX Reference for PostFiat

Status: reference analysis only. Do not copy Phantom assets, typography, logos, copy, or brand colors.

This document records public Phantom wallet screenshots and describes the design logic precisely enough to build a PostFiat-native wallet mock later. The goal is to learn from the product structure, hierarchy, and interaction patterns, not to clone the interface.

## Source Packet

Reference screenshots were captured into an operator-local archive outside the
repo. They are not source artifacts and are not required to build or run the
wallet.

Important files:

- `phantom-google-play-reference-sheet.png` - local contact sheet of all captured panels.
- `screens/GP-01-money-favorite-app.jpg`
- `screens/GP-02-home-portfolio-buy-sell.jpg`
- `screens/GP-03-buy-keypad.jpg`
- `screens/GP-04-phantom-cash.jpg`
- `screens/GP-05-card-spend-send-receive.jpg`
- `screens/GP-06-social-token-feed.jpg`
- `screens/GP-07-profile-activity-chart.jpg`

Public sources:

- Official Google Play listing: `https://play.google.com/store/apps/details?hl=en_US&id=app.phantom`
- Official Phantom site: `https://phantom.com/`
- Official receive help: `https://help.phantom.com/hc/en-us/articles/4406393831187-Receive-tokens-in-Phantom`
- Official send help: `https://help.phantom.com/hc/en-us/articles/5530158379539-Send-tokens-from-Phantom`
- Official cross-chain swapper post: `https://phantom.com/learn/blog/cross-chain-swapper`

## Measurement Notes

The captured Google Play panels decode locally as `333 x 592 px`. This is a narrow mobile preview ratio of about `0.5625`, equivalent to `9:16`.

These screenshots are marketing panels rather than raw device captures. That means:

- Layout, hierarchy, palette, and component proportions are useful.
- Exact in-app animation timing cannot be proven from these static images.
- Transition guidance below is marked as either screenshot-derived or implementation guidance inferred from common wallet flows.

## Palette Measurements

Sampled by browser canvas from the archived local images.

| Screen | Size | Dominant Sampled Colors | Interpretation |
|---|---:|---|---|
| GP-01 | 333x592 | `#a090f0`, `#b0a0f0`, `#e0e0ff`, `#b0b0ff` | Lavender/blue brand field with soft pale highlights. |
| GP-02 | 333x592 | `#101010`, `#202020`, `#b0a0f0`, `#b0b0f0`, coral/red accents | Black app surface inside purple/coral gradient marketing shell. |
| GP-03 | 333x592 | `#101010`, `#c090ff`, `#b0a0f0`, `#6090f0`, coral/red field | Buy flow is mostly black UI with one lavender primary CTA. |
| GP-04 | 333x592 | `#ffffff`, `#6090f0`, `#5080f0`, `#709ff0` | Cash/card marketing panel, not a dense wallet app screen. |
| GP-05 | 333x592 | `#101010`, `#202020`, `#405fb0`, `#5050a0`, `#303030` | Card/settings UI uses dark rows on blue field. |
| GP-06 | 333x592 | `#101010`, `#404080`, `#404070`, `#405090` | Token feed is dark with muted indigo social background. |
| GP-07 | 333x592 | `#403060`, `#101010`, `#202020`, `#404070`, `#9080e0` | Profile/activity screen uses dark cards over purple field. |

PostFiat color mapping must not use Phantom purple as the primary identity. Use the postfiat.org palette instead:

| Role | Phantom Pattern | PostFiat Equivalent |
|---|---|---|
| App background | Near-black `#101010` | `#000000` / `#060806` |
| Card surface | Dark gray `#202020` | `#101410` / `#151b15` |
| Brand accent | Lavender `#b0a0f0` | Signal green `#7fee64` |
| Muted copy | Gray/purple gray | Pale secondary `#bad8b6`, muted `#83917f` |
| Positive PnL | Green | Green remains, but lower saturation for dense rows |
| Warning/risk | Coral/red | Coral only for risk, not decoration |
| Dividers | Low-contrast dark gray | `rgba(221,255,220,.14-.18)` |

## Screen GP-01: Brand/Balance Hero

Source file: `screens/GP-01-money-favorite-app.jpg`

Visible structure:

- Full 333x592 panel is a lavender background with soft shape layering.
- Phantom logo sits near the top center, occupying about `35-40%` of panel width.
- The central balance card is rotated slightly and visually floats over a ghost/lock illustration.
- The money amount is the main information object, around `12-14%` of panel height.
- Headline sits in the lower third, center-aligned, large and heavy.
- Supporting copy sits below headline, center-aligned, smaller and lower contrast.

Design logic:

- The app is positioned as a money home before any specific operation.
- Balance is used as the primary object; security is suggested through lock imagery.
- There is no visible navigation or control density. This is onboarding/marketing composition.

Do not copy:

- Ghost mascot.
- Lavender identity field.
- Exact headline style.

PostFiat adaptation:

- Use a measured "verified balance" hero for onboarding only.
- If shown in the product, title the number by rail: `Total PFT`, `FastPay`, `Shielded`, or `Verified NAV`.
- Use the PostFiat black/green grid field, not a purple mascot treatment.

## Screen GP-02: Home / Portfolio

Source file: `screens/GP-02-home-portfolio-buy-sell.jpg`

Visible structure:

- Marketing background is a warm lavender-to-coral gradient.
- Phone UI is centered and takes roughly `68-72%` of panel width.
- App surface is near-black.
- Top bar contains avatar/account label on the left and two utility icons on the right.
- Primary balance is shown near top, large, left-aligned.
- Positive daily delta appears directly under the total, smaller and green.
- Four action buttons sit in a single row: `Receive`, `Send`, `Swap`, `Buy`.
- Below actions is a cash/balance promo card with a right-side CTA.
- A `Predictions` strip appears as a secondary section.
- Token list begins below with rows containing icon, asset name, amount, fiat value, and green delta.

Quantified layout logic:

- Header zone: about `8-10%` of app height.
- Balance block: about `12-15%` of app height.
- Action row: four equal cells, each roughly `20-22%` of app width, with about `8-10 px` gutters in the captured scale.
- Asset rows: roughly `48-60 px` high at captured scale.
- Text hierarchy is strict:
  - Balance: largest type in the screen.
  - Asset symbol/name: medium.
  - Fiat value and deltas: smaller but high contrast.
  - Supporting labels: muted.

Interaction logic:

- Tap `Receive` opens an address/QR receive flow.
- Tap `Send` opens recipient selection and amount entry.
- Tap `Swap` opens a token-pair flow.
- Tap `Buy` opens a fiat/deposit flow.
- Tap token row opens asset detail/activity.
- Tap account/avatar opens account/profile selector or sidebar.
- Tap search opens asset/user/action search.

Transition guidance:

- Home-to-action should be a push or sheet transition, not a route reload.
- Balance remains visually stable after returning from a child flow.
- Token rows should update in place after data refresh; avoid full-screen flashes.

PostFiat adaptation:

- Home should use the same hierarchy but with PostFiat rails:
  - `Total PFT`
  - `Account Balance`
  - `FastPay Balance`
  - `Shielded Balance`
  - `pfUSDC`
  - `a651 NAVCoin`
- The four primary buttons should be `Receive`, `Send`, `FastPay`, `Shield`.
- Secondary row can hold `Swap`, `Bridge`, `Run Proof`, `Activity`.
- Delta under total should be replaced by status: `WAN devnet height`, `RPC connected`, `finality healthy`, or `proof fresh`.

## Screen GP-03: Buy Amount Keypad

Source file: `screens/GP-03-buy-keypad.jpg`

Visible structure:

- Background is coral-to-blue gradient outside the phone.
- Phone UI is black, with a small header at top.
- Header contains asset/action context: `Buy BTC`, verification mark, and social proof line.
- Main amount is centered and very large.
- Funding source row sits below amount: `Pay SOL`, source amount, and swap/route affordance.
- Primary CTA is a full-width lavender button above keypad.
- Numeric keypad occupies the lower third.

Quantified layout logic:

- Header/action context: top `10-12%`.
- Amount display: central `25-30%`; visually uncluttered.
- Funding row: `36-44 px` high.
- CTA: `40-48 px` high, full width with side margins.
- Keypad: three columns; each key has at least `44 px` target height in the captured scale.

Interaction logic:

- Numeric entry dominates; no visible cursor-heavy form field.
- CTA only becomes meaningful once an amount is valid.
- Funding source is a compact row that can open a selector.
- Asset context is persistent while entering amount.

Transition guidance:

- Entry should animate numbers in place, not move the form.
- Invalid values should disable CTA and show inline copy near the funding row.
- Funding source selector should open as a bottom sheet or side panel, not replace the whole amount screen.

PostFiat adaptation:

- Use for `Send PFT`, `FastPay Send`, `Bridge In`, and `Swap`.
- Replace asset header with explicit source lane:
  - `Account -> recipient`
  - `FastPay owned object -> recipient`
  - `pfUSDC -> a651`
- The amount screen should show expected latency:
  - `Account finality: ~1.5s`
  - `FastPay certificate: ~50-200ms`
  - `Shielded proof: prover required`
- Keep the keypad/amount-first shape, but use PostFiat green CTA and black surface.

## Screen GP-04: Cash Product Hero

Source file: `screens/GP-04-phantom-cash.jpg`

Visible structure:

- Blue gradient field.
- Large centered headline.
- Card illustration and balance card.
- No dense controls.

Design logic:

- This is an account feature promotion, not a transaction screen.
- It uses a soft physical-card metaphor to make cash/debit features legible.

PostFiat adaptation:

- Do not create a similar marketing card inside the wallet.
- The useful idea is a separate "cash-like" pocket, which maps to `FastPay Balance`.
- Use compact wallet rail cards, not marketing hero copy.

## Screen GP-05: Card / Spend / Send / Receive

Source file: `screens/GP-05-card-spend-send-receive.jpg`

Visible structure:

- Blue/purple background.
- Phone card screen uses black surface.
- Top bar has back control and screen title.
- Payment card visual occupies the upper third.
- Settings/action list rows sit beneath: show details, available to spend, freeze card, change PIN.
- Floating send/receive tiles overlap the phone near bottom.

Quantified layout logic:

- Back/title top bar: `40-48 px` at captured scale.
- Card preview: about `30%` of phone content height.
- Rows: `44-52 px`, one-line labels, chevron on right.
- Toggle control is embedded into row, aligned to right edge.

Interaction logic:

- Sensitive/card operations use list rows, not custom controls.
- Destructive/security action (`Freeze Card`) is a visible toggle, not hidden in settings.
- Send/receive affordances are promoted as paired actions.

PostFiat adaptation:

- Use similar row mechanics for wallet security:
  - `Auto-lock`
  - `RPC endpoint`
  - `Export backup`
  - `Remove wallet`
  - `FastPay owned objects`
  - `Shielded notes`
- Do not use a physical card unless there is a real PostFiat card product.

## Screen GP-06: Social Token Feed

Source file: `screens/GP-06-social-token-feed.jpg`

Visible structure:

- Phone screen shows token header with close/search/settings icons.
- Token price is compactly displayed in the header.
- Feed rows look like chat messages with handles, text, and colored event rows.
- Buy/sell events are highlighted with green/red row backgrounds.

Design logic:

- Phantom combines market and social context in the same asset detail view.
- Activity is chronologically dense and conversational.
- Important action events get color-coded strips.

PostFiat adaptation:

- Do not add social feed unless it exists.
- Useful pattern: asset detail/activity feed for `a651`, `pfUSDC`, `PFT`.
- Event rows should show:
  - `account transfer`
  - `FastPay certificate`
  - `shield ingress`
  - `private swap`
  - `private egress`
  - `bridge relay`
  - `NAV proof`
- Color rules:
  - Green: completed credit/inbound/proof fresh.
  - Pale/neutral: read-only snapshot.
  - Amber: waiting/needs action.
  - Red: failed/rejected.

## Screen GP-07: Profile / Activity / Chart

Source file: `screens/GP-07-profile-activity-chart.jpg`

Visible structure:

- Purple dark background.
- Top profile panel with avatar, handle, metadata, and follow button.
- Recent activity list begins below.
- Token chart card appears as an embedded preview with a `Buy` button.

Design logic:

- Identity, social proof, activity, and asset action are combined.
- Activity rows include time metadata.
- Chart card is visually nested but still compact.

PostFiat adaptation:

- Use for account profile/detail view, not public social identity:
  - wallet address
  - public key fingerprint
  - chain/network
  - latest finalized height
  - recent activity
  - proof freshness
- Chart-style card can show NAV history for `a651`, but only if it reconciles to real proof history.

## Phantom Pattern Summary

The strongest reusable UX patterns are:

1. One dominant value at the top of the home screen.
2. Four short primary actions immediately below the value.
3. Dense but readable asset rows with icons, balances, fiat values, and status.
4. Amount-first transaction entry with a native keypad feel.
5. A compact funding/source row instead of a long form.
6. A clear review step before irreversible actions.
7. Activity as a timeline of human-readable events.
8. Security and settings as simple rows with toggles/chevrons.

The patterns that should not be copied:

1. Phantom purple identity.
2. Ghost mascot.
3. Social/trading emphasis if the feature is not actually present.
4. Marketing panel composition inside operational wallet screens.
5. Any Phantom-specific copy, icons, badges, or brand typography.

## PostFiat Design Rules Derived From This

Canvas:

- Mobile-first wallet shell: target `390 x 844 CSS px`.
- Browser/web shell: center the same wallet panel at `420-460 px` width, with optional right-side detail pane above `960 px`.
- App background: `#000000`.
- Wallet panel background: `#060806` to `#101410`.

Typography:

- Use one large balance number per screen.
- Balance number: `40-48 px` mobile, `44-56 px` desktop panel.
- Section labels: `11-12 px`, uppercase optional only for technical labels.
- Asset rows: `15-16 px` asset name, `13 px` metadata.
- Monospace only for hashes, addresses, proof IDs, and heights.

Spacing:

- Screen horizontal padding: `18-22 px`.
- Primary action row gap: `8-10 px`.
- Action tile height: `58-66 px`.
- Asset row height: `56-68 px`.
- Review rows: `44-52 px`.
- Bottom CTA height: `48-52 px`.
- Border radius: `8 px` for cards/list containers; `12-14 px` for buttons/action tiles; circular icon buttons only where icon-only.

Color:

- Dominant surface: black/near-black.
- Accent: `#7fee64`.
- Text primary: `#ddffdc`.
- Text secondary: `#bad8b6`.
- Muted: `#83917f`.
- Lines: `rgba(221,255,220,.14-.18)`.
- Warning: amber/coral, reserved for real risk or incomplete state.

Motion:

- Screen push/sheet transition: `180-240 ms`.
- Use `ease-out` for entering panels and `ease-in` for exiting panels.
- Balance refresh: crossfade number over `120-160 ms`; do not slide layout.
- Toast: bottom or top-in-panel, `220 ms` enter, auto-dismiss after `3-5 s`.
- Respect `prefers-reduced-motion` by disabling transform transitions.

PostFiat Wallet Home:

- Top bar:
  - left: wallet avatar/fingerprint
  - center/left text: account label
  - right: network status and settings/search icons
- Hero:
  - `Total PFT`
  - status line: `WAN devnet height H`, `RPC connected`, `finality healthy`
- Action row:
  - `Receive`
  - `Send`
  - `FastPay`
  - `Shield`
- Secondary rail cards:
  - `Account Balance`
  - `FastPay Balance`
  - `Shielded Balance`
  - `pfUSDC`
  - `a651 NAVCoin`
- Activity:
  - recent events grouped by rail and finality/proof status.

PostFiat Send Flow:

- Step 1: choose source lane (`Account`, `FastPay`, `Shielded`) as segmented control.
- Step 2: amount-first entry with selected lane and available balance visible.
- Step 3: recipient entry/selection.
- Step 4: review:
  - `You send`
  - `Recipient receives`
  - `Fee`
  - `Speed/finality`
  - `Privacy impact`
  - `State root / certificate / proof ID` under advanced.
- Step 5: result screen with receipt and activity link.

PostFiat Swap/Bridge Flow:

- Keep Phantom's amount-first pattern, but expose proof/finality facts:
  - `pfUSDC -> a651`
  - `Public -> shielded`
  - `shielded -> public`
  - prover status
  - certificate status
  - expected route and fallback state.

## Open Items Before PostFiat Mock

1. Capture actual Phantom extension screenshots if we want extension-specific dimensions. The current packet is mobile/app-store-oriented.
2. Decide whether the PostFiat wallet mock should be:
   - mobile-first full-screen web app,
   - browser extension popup,
   - desktop panel inside StakeHub,
   - or all three breakpoints.
3. Decide whether FastPay send should remain disabled in the mock or be represented as a design-ready flow with a disabled final submit.
4. Decide whether NAVCoin proof history belongs on Home or a dedicated `NAVCoin` tab.
