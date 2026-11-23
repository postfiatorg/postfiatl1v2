# PFSH02 M8 release rehearsal

**Release held.** The candidate verified the full R64 archive and the fresh six-validator history. The M4 node rejected the signed height-2 amendment while validating the fresh five-block pre-activation prefix. A live activation height requires a compatible upgrade path, independent review and a founder decision.

| Input | Exact revision or SHA-256 |
| --- | --- |
| Node and specification under test | `f0a44680e6435fb87a7392f75677a86d3ba5d5fe` |
| Rehearsal source | `e7fa39058489b7a237ca98c008392099eac378c2` |
| Original R64 executable | `674dbb18a29931d9ae749d28270e3c6cc508795a`, binary SHA-256 `95d5b237c991f57a9ea122e110f6a2d0c017bf9ca0a85315b2359ecba3b017d1` |
| M4 node, before activation support | `b8c8a44031090f4965e9f735fa3348fe010907d1` |
| M4 release executable | `9bcf884221c809569dc0b6cbdde6d0c73d0d229a6686567815a31b200f58ba65` |
| Candidate release executable (pq-wallet feature) | `5a51087560361e1577ac30ea0780589038275db7ebedfe3497bf280bc554d772` |

Shared x86-64 KVM host, AMD EPYC, 32 logical CPUs, two threads/core, 32 MiB L3. Rust/Cargo 1.95.0, Go 1.25.1, Python 3.12.3, Z3 5.4.1 and Node 22.23.1. The chains below use base units. Each asset family total includes its child series.

## Exact R64 archive

Validator-3, height 1 through **1,734**. Both executables replayed from genesis in isolated copies of the retained transactional directory. The original R64 node, the isolated M5 check and the two-binary comparison returned the same digest for the height/hash/root rows.

| Field | Result |
| --- | --- |
| Genesis | `ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9` |
| Final block | `910c744852de4203274bd606859edaccdc27e25181223a1dd07cf2f62da3de048ffef8e427dcf229a00cbb8225121e40` |
| Final state root | `a7b1185e94bf0e224a3944be4aaba896618594823ba2225399dad2148f6611fe527ce6a60a7852f94c8bc154a90dfe56` |
| SHA-256 of all 1,734 rows | `c28eb87280050e96e996ad5c9e8c2eae12f2fb9774438f3a7b5f1295ef155910` |
| PQ activation, sizes and time | No H, no pool pairs, no PQ bodies or proofs to measure |
| Native PFT | 999,954,296 live, 1,000,000,000 at genesis, 45,671 receipt fees and 33 fees on retained consensusless FastPay certificates |

| Unit | Definition | Asset ID | Total base units |
| --- | --- | --- | ---: |
| PFUSDC | family | `02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b` | 5,740,471,455 |
| a651 | family | `42cb574dcf156f906ce424f8555eac83c7dde33e48c44f4ef6307df705edfe036f69b1d437e8b7c630dd325684965a0c` | 0 |
| a651 | family | `85ed5a81cd7a4c662fcf45d1e2e5746827b146dbf370f9421035a31493fc0741891d76d255bce17ddf0d7c6fc50c5859` | 0 |
| a651 | family | `de8d3d78aca83f7000632cbd3dd06edd30d105085d16eef1696f9e1707908ec9209239080d21ca1c0cecc36ca6f966b6` | 45,702,464,561 |
| a666 | family | `300bf48a63a94770b6e67817f88cd1abf77e7f592a061e15682d7fd9973260af4c2e631e32df3c2c402b7d2fe272a293` | 200,000 |
| A666 | family | `521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c` | 31,519,438,511 |
| PNOK | family | `4b8dd69b1f1ee425ae84d17f2fc2fe3630d904c4c356b0ff2df0e00c20109e6346d765ca4c0407ddb08edcb6525d5cc1` | 500 |
| PFUSDC | series | `c6d2a21646091dc721332d114f6288ede715d0a6325b3cf41ec7b4865f9f2e85014e79445fb66a7141747e4f42584062` | 14,000,000 |
| PFUSDC | series | `3923511d5be0557a61051e099b606d3decc11a5ba274c7d551168735accad8ed18d89c9200efc4bfbbbab6e85d4c173f` | 0 |
| PFETH | family | `629f1ba8f36963662d6e36243aea85627787cd72d41a9e92c5f01b60e3e68c1b951af771d16780bf13c4036b8d2a4996` | 0 |
| PFETH | series | `418c0dd528239758d2b684cd9fd06e066aefc5f4cc231a796a279e8cc0a31ad9e3bd474eb1c22847059300a2cce2bae2` | 0 |
| PFUSDC | series | `2bae082a6703375b9405af44715e1e64623265392627767b040fa2c30abb100a09da403724a6f105317292d9c0073df7` | 75,067,140 |
| BMNRC | family | `43b9ba500a8cc04b13a10f35c3187ff09fdf52a3accfb507226144dcbe2ef14f4b2e1d555c62bbdf63e5cf3e7e8231cc` | 640,585,238 |
| PFDEP | family | `1489e9672c5901432631094e5b2070291f7441820651f0cae3946131d62eeca3753eced417af6e37928c0554cafc76b6` | 0 |
| B23 | family | `ca7845134c8e95819082fcd2f1612808e20a37dfabab98dab120f99fd2c54c18ffb50d98c0664480420c881773244627` | 1,000,000 |
| PFUSDC | series | `541e451e6ae96b6680988655571a0feec59c7f1d073c9b5d29547254cab38a3be540f8767446d25baebc7081ce97aef7` | 5,354,189,330 |
| B24 | family | `52285e4ded1d58113cfaa12bbc034b8b10213f699b3224c74160450229d6bf7ccfe505c78f2599d975ae2ecf98f7d8af` | 425,323,380,000 |
| PFUSDC | series | `e08c9bc78cbff20f45d84c05120b99d39674237438d5af53d62387709fa57fe3a711e084773aa7eb6cb215278dadf48e` | 7,303 |
| PFUSDC | series | `aecd46d679c89533794668dcc6ee3c28ca08313603643b2d488e1fb3d7e80e80df47d51ca1e80a7339a9eeb3624b5194` | 5,303 |
| PFUSDC | series | `c843a071e81cbb2e6dfaccd5e9d4a95d23ec36f3da7a8bb1af4199e5a24e22a524d0ff4a46c6a79c7136fc9d82e875f1` | 6,104 |
| PFUSDC | series | `19d06de09e8d4926112ac93b7ee95136f5aebb4631f0aaf3cde6c9383d5a684e4b5f9f364c951ba5f830ed66ad0bf106` | 59,745 |
| PFUSDC | series | `f8b846d762c5ae2a6340271c273d37202de2be9f65f0330cb4d5b76f9409101918a54528ce0632e0a19ea78fb6237c8e` | 353,539 |
| PFUSDC | series | `1f2048bed45e80cee0e55e5b466bc4f64376560fdfc952a8604608f89713aea2abfad25774726b97f0f1e82cf310288a` | 1,031,452 |
| PFUSDC | series | `baffc58ea2a9e03f15a0f9416b2e5ac0b0d1d5d5d1ee0ff1992b1a1ccc9b84c8043fe5662a474c926ab18a0ab8d1e3dd` | 123,407 |
| PFUSDC | series | `6cb83042de6e869ca412d3ff7cc3fc6b48acc8a332e1cdbee651cbb78d769ea88f88a44b5098390c069df45dbb9ac8c3` | 505 |
| PFUSDC | series | `fc22fc6ddffebddb5411133d7dd3c8c4b59b65f81e822ad8dd270a41ccc1c193dd10165920f5fe88a0a6c9c4069dff87` | 430,000 |
| PFUSDC | series | `7039e36cf4964085f6a847ee4c7644df39db905d3ff06f7331abaa5c73d0f86a1cffba9ead914a1cf9b101bdfb75444b` | 127,003 |
| PFUSDC | series | `68c8a42305516e1d3229de72a26a68d1824118f9d63d58bc4c74e0151e49cd4a35c650a6f8bd03b73d9aba6b514df552` | 80,021 |
| PFUSDC | series | `873b06ac4ab8a8e06f834a71b23d012b1ddc9d58f2ef19136bb69702639ef1fbbd1acad18b98739b10e39cc247974c02` | 20,008 |
| PFUSDC | series | `8afde29570a3b7f4392860b7dffeb9a9605e5578322c8a1ff611ac933476dd5e7a1b57c88c7ae6ddc55c0ed1ae07112c` | 50,000 |
| PFUSDC | series | `541cb1bb58a792ef337fcf88351c7ff519a979fe49f5d2c194334d1141e5758b04c114a1efbf1b478e07fdc78c42a201` | 5,180,000 |
| PFUSDC | series | `fd660c0c7bdc912396cd36fca37cc09a91cef46d53ffbd1446e3425a68c24cab9bc0c10ed2b279ff8df27fafc7c6de8b` | 40,000 |

The Orchard pool report on this same R64 copy passed. It counted 177 outputs, 165 nullifiers, 137 retained roots and 41 accepted anchors. Its latest retained Orchard root was `d55447e9ae66194069fe6bff4ff113f9ab83d0a1cafa756281bf801f17972935`.

## Fresh six-validator chain

Disposable genesis `e9c735eecfc4550f152330b52f782de84eea2aa7d59dc7e9e317718ed2ba28473854f0cb7c6c972f7f2825ba944b72e8`. The chain certified ordinary traffic at heights 5 and 8, PFT payments at 6 and 7, four proofs at 9, and a 100-unit pfUSDC source-series deposit at 28. The bridge observation, reserve and claim were created with signed local test transactions. The fixture begins with two signed governance blocks, funding and an asset setup block on its bootstrap node. It replicates that history, isolates each validator signer and exercises heights 5–30 over loopback with a five-vote quorum. Consensus-v2 activation is outside this 30-block fixture. The certified header admits one batch per block. Ordinary traffic and PQ therefore alternate across blocks.

| Field | Result |
| --- | --- |
| Earliest PQ inclusion | H = 6, scheduled in block 2 |
| Pre-H prefix | 5 blocks, row SHA-256 `154f98e1eb481c589d9771d8b65a01b1fd384e3a1ab1871f29bc391afc8cb6c1`, candidate state root `e61b4cfc991788d4e309535e2188443c94ed8de4406b4e52bf8e9a99cd909f814500dc04bd1c7a92d08000e252746777` |
| Finalized pause | Block 29 records value zero. New PQ inclusion rejected at candidate heights 30 and 31. |
| Final block (30) | `dc963b2bef20e0f6541a7ee6ffc074ee9c080c159c1f2cde2c3fcc6f48f0df8e7de6033460aa93c234dc9479826192dd` |
| Final state root | `d26fd217c54d1ac47ec3a4694acf8eeb425eaef846e4a1f35fd485d423d107bae8702c339bf1094ee4d2e96cb46373ba` |
| Largest stored body / charged payment | 144,397 / 145,140 bytes (149,130 charged ceiling) |
| Full PQ block | 4 proofs, 578,304 charged bytes (596,520 ceiling) |
| Single uncached L1 verification, observed maximum | 40.259 ms over 7 live and 7 restored read-backs |
| Ordinary / four-proof round | 844.65 / 979.44 ms including certification and persistence (20,000 ms vote phase) |
| Native PFT | 999,999,413 live + 587 burned = 1,000,000,000 genesis |

| Unit | Definition | Asset ID | Total base units |
| --- | --- | --- | ---: |
| A666 | family | `9f7a9e7b5fe8a4a7f26aec66c9fddfe1817c96cbc141f85a712b548149dda5b3b541736f14c8caa4c9d22b97e4e095a0` | 200 |
| USDC | family | `82c3308ffdaf0df2d30fcd8d2a508dc99719777c2822424986e194a6902af1fd3ebe698d87dca89f711a02be7bb1426b` | 200 |
| pfUSDC | family | `d48ccde2847cd40aa54d1a2a292c6f4dd714f2c72dc0d79584c7d308969977122fcb7d4426af3a5c195d635d752410c9` | 100 |
| pfUSDC | series | `bbf2fb96437f2e87ff3b470f19aa2455ae226ab810b06b64ab327b0daa71a2d1ad14638a107b7bbd3b0cf3a6f4d56269` | 100 |

| Pool | Epoch | Reserve | Final note root |
| --- | ---: | ---: | --- |

| PFT | 0 | 494 | `22481c24c248a8ec94f1070211bd8b695a9c5a0ddd4112d55be2ffcff05f171700d9d6458c935547c7a85b16f841108e` |
| pfUSDC source series | 0 | 100 | `b62742dc5bb69ce28794992f720fb3834f66e3f80c82182ca06d06f671723839e9249ae13c128c561dc7ddc56e5f7034` |

The candidate replayed all 30 blocks on all six replicas and a separate signed-snapshot import, then cold-started its CLI separately on the original replica and the import. The trusted publisher public key was exported before import. The restored wallet decrypted a 99-PFT note and a 100-pfUSDC note and checked their serials remained unspent. The identical native deposit proposed successfully at 29 before the pause and failed at 30. The seven finalized receipts and stored proof bodies remained readable and independently reverified on the restored node.

| Height | Asset | Payments | Accept votes / validators | Certificate ID | Accepted receipt IDs |
| ---: | --- | ---: | ---: | --- | --- |
| 6 | PFT | 1 | 5/6 | `2e810bee4434657ec2829fac752e99912cbe4aa5f7714c8b3b0e3a318a91f44226cb01be5e96894ea7f3a3378b5c9f0a` | `e9d0fa1287699e5b8b465ded553103a474d33efdc2d5d5731aff38488235788aeeded3eea9aec49b1555b0fe99637343` |
| 7 | PFT | 1 | 5/6 | `90ecd1fa1992c94e7216bcdb9867f368b2b55cd02a21e3458c1b3ee70d2c5649d9acd53f5b48485ccfec83255aeb612f` | `cbcc41c9ac129aff9fd52deb80e5ea2cf07ac823cf5edef7ad8007ec9cd1025d33861d06156f9010ee554aaf6c44d6f0` |
| 9 | PFT | 4 | 5/6 | `170dbb5ca4baf8347723c7285c46115f68a3bacc76a149e0380e020a289a7b471317be859e059932d1540295c824347c` | `3b9c0cbab27a9aa4fae5842b708797f789dc3ffb5f066f6dde941337912407ae724f58f46ea77dfaf607417206fb4938`<br>`fef82200f5e047195827b705950d53927a7f5db2cb567ad6e01a7c6c05053dc93b53f137250f170fb8b965a62923f519`<br>`c5f56980665dd09686d4a6dd9e06efe27c3ed1a689db345e7554b42d8043d65ca5a9158b535cdf48f508fba11e7005a4`<br>`2a03bd30143d96da5fa4fbef66b08b8be8dbe5d936d2005adb2da0a1a7bd2c949572ab9b72213a1fad62a87b125244e7` |
| 28 | pfUSDC | 1 | 5/6 | `4e359a481b37b5df4d3e7abc3a014840d38e3e0fd7303bbcb756f28f0b2491a353f44abcb76b924092cd5f423c39b594` | `fd4d57a1d2a110256c65e37467541ffe8c8f348b5e4c7546cb7394f1f4504e656073b902fbca2fb209ebaa727b8ec60a` |

The rollover test starts two authenticated frontiers two leaves below capacity, verifies both L1 boundary proofs and a spend from the frozen epoch into the successor. The regression suite separately exercises the persisted state transition and independent Go reconstruction. The standalone replay builds all 16,777,216 leaves. The 30-block fleet did not generate a full tree of live deposits.

| Epoch | Leaves | Terminal root |
| ---: | ---: | --- |
| 0 | 16,777,216 | `bc84ead5ae686c067739f7653f2a7959f7f61e1f23d765dda23b53b7c598d7d89e4602c22f154bcaba0e5a782f79e2b1` |
| 1 | 16,777,216 | `fc071c3d095be6479dab94a7f55fdcd41377c55083dcdbf73cdbe0996aca71a261c81dad2afcab7a41c22fff8b6b8cbd` |

## Upgrade failure

Both executables returned byte-equivalent decoded blocks 1–5 from the fresh archive. The candidate recomputed their hashes and roots. The M4 binary stopped on the new governance kind in block 2, before H. This prevents certification of the requested rolling upgrade. Identical reads alone cannot establish that old validators accept those blocks.

```text
error: verify-blocks failed: unsupported governance amendment kind `pq_payment_rehearsal_v1`
```

## Reproduction

Run from the rehearsal revision above with Bash, Cargo, Docker, Ubuntu 24.04, the supplied exact R64 directory, and a fresh working directory. Supply absolute paths for the archive and work directory. Keep both builds in the same absolute worker target directory and copy each completed executable before starting the next build.

```bash
export CARGO_TARGET_DIR=/home/postfiat/var/pq-work/target-m8
mkdir -p "$WORK" "$WORK/tmp"
export TMPDIR="$WORK/tmp" RAYON_NUM_THREADS=4
git clone --no-checkout . "$WORK/m4-src"
git -C "$WORK/m4-src" checkout --detach b8c8a44031090f4965e9f735fa3348fe010907d1
(cd "$WORK/m4-src" && cargo build --locked --release --target x86_64-unknown-linux-gnu -p postfiat-node --bin postfiat-node)
cp "$CARGO_TARGET_DIR/x86_64-unknown-linux-gnu/release/postfiat-node" "$WORK/m4"
git clone --no-checkout . "$WORK/candidate-src"
git -C "$WORK/candidate-src" checkout --detach f0a44680e6435fb87a7392f75677a86d3ba5d5fe
(cd "$WORK/candidate-src" && cargo build --release --locked -p postfiat-node --features pq-wallet --bin postfiat-node --bin pftl-pq-walletd)
cp "$CARGO_TARGET_DIR/release/postfiat-node" "$WORK/post-h"
tools/pft-binius-shielded-pq/replay-r64.sh "$R64_ARCHIVE" "$WORK/m4" "$WORK/post-h" "$WORK/paired-replay"
cargo test --release --locked -p postfiat-node --bin postfiat-node -- pq_payment_rehearsal_tests --nocapture --test-threads=1
PQ_PRE_H_NODE="$WORK/m4" PQ_POST_H_NODE="$WORK/post-h" cargo test --release --locked -p postfiat-node --bin postfiat-node -- pq_payment_rehearsal_tests --nocapture --test-threads=1
```

The last command returns a failure on the recorded revisions. Fresh block IDs and state roots can vary across separate signed rehearsals. The table above refers to one run. To inspect the R64 supply, build `cargo build --release --locked -p postfiat-node --example pq_rehearsal_inventory` and mount the compiled inventory executable and a scratch copy of R64 into the same validator-3 container path used by the paired replay script. Local archive owners can also run `target/m5/check-meter.sh archive` in a scratch layout with the original M3 baseline and archive-check helpers.

## Resource and regression gates

Validator measurements include parsing, proof and account verification, state lookup, append and accounting. Each profile used 21 samples per asset, acceptance/rejection and warm/cold condition with 1,024 history entries. Each row gives the highest per-condition statistic across both assets. Persistence is measured separately. All wall-clock values are milliseconds.

| Backend and pinned CPU | Actions | Result | Median | p95 | Maximum |
| --- | ---: | --- | ---: | ---: | ---: |
| Accelerated 17 | 1 | Accepted | 25.37 | 30.96 | 31.49 |
| Accelerated 17 | 1 | Rejected | 24.35 | 36.58 | 36.60 |
| Accelerated 17 | 4 | Accepted | 143.58 | 166.13 | 166.23 |
| Accelerated 17 | 4 | Rejected | 95.31 | 130.45 | 138.67 |
| Portable 13 | 1 | Accepted | 38.96 | 46.51 | 50.23 |
| Portable 13 | 1 | Rejected | 39.79 | 44.11 | 47.55 |
| Portable 13 | 4 | Accepted | 154.56 | 162.63 | 186.54 |
| Portable 13 | 4 | Rejected | 159.54 | 252.45 | 252.53 |
| Portable retry 11 | 1 | Accepted | 39.16 | 48.26 | 54.50 |
| Portable retry 11 | 1 | Rejected | 40.08 | 53.18 | 55.32 |
| Portable retry 11 | 4 | Accepted | 200.88 | 234.42 | 235.86 |
| Portable retry 11 | 4 | Rejected | 150.99 | 164.58 | 166.62 |

| Backend and pinned CPU | Durable commit median | p95 | Maximum | Whole-process peak KiB |
| --- | ---: | ---: | ---: | ---: |
| Accelerated 17 | 48.02 | 67.73 | 67.73 | 619,340 |
| Portable 13 | 45.68 | 66.95 | 66.95 | 619,088 |
| Portable retry 11 | 49.30 | 53.58 | 53.58 | 619,344 |

The 50 ms single-action engineering gate was missed on the first portable trial and on the idle-core rerun. On CPU 13 the cold pfUSDC acceptance reached 50.23 ms. On CPU 11 cold pfUSDC acceptance/rejection reached 54.50/55.32 ms. The four-proof CPU budget of 200 ms was also exceeded on these samples. The complete certified fleet round is timed separately above.

The independent seven-stage replay on CPU 5 passed 7/7 on the first run. Full-tree Rust parse/verify/apply took 20.47 ms and Go parse/verify took 31.02 ms for the same 138,364-byte payment. The standalone driver includes corrupted inputs, mint authorization, independent scanning, arithmetic bounds and circuit equivalence.

The Python CLI completed its eight certified payments and restored both assets. The browser test completed two isolated sessions and confirmed the migration from 9 Orchard units through the public account into PQ custody. The four-test six-validator transport selection initially reported an unsupported hello schema, then passed all four on CPU 26. The first offline workspace build lacked a locked registry dependency. The worker report includes individual commands and initial diagnostics.

The full workspace run reported 2,451 passes, four assertion failures and 53 existing ignored tests. Three external-lane release/export cases expected a 1,000-unit fee and a fee-charging receipt code. The node returned zero and an uncharged rejection. A fourth test found a different default RPC systemd unit digest. The same selections failed in the retained M7 test executable. These failures keep the release gate open. During this run 25 Rust documentation targets also reported missing crate metadata while other builds were using the worker target directory. After those builds finished, `cargo test --release --workspace --locked --offline --doc --no-fail-fast` completed successfully for all 26 documentation targets. The four assertion failures above remain open.

The focused M1–M7 rerun passed. The selections overlap and should be read individually.

| Selection | Result |
| --- | --- |
| Activation, reservation and private-action admission | 16 passed |
| Gateway, local-chain fleet, pool/snapshot and broader node regression selections | 3, 5, 4 and 66 passed |
| M1 and M2 supplemental negatives | 4 and 2 passed |
| Accelerated and portable Rust L1 verification | 2 unit and 5 vector tests passed on each backend |
| Rust/Go L1 and rollover fixture suites | Both Rust regenerated vector directories matched byte for byte. Go's accelerated and pure-Go suites agreed on both frozen roots. |
| Types, execution, storage archive and storage recovery | 260, 291, 5 and 12 passed |
| Account transaction RPC, pool page/snapshot, Cobalt governance | 3, 3 and 14 passed |
| Affected Orchard/governance negatives and issued-asset ingress/egress | 4 and 1 passed |
| Browser wallet, Chromium custody, public browser and Orchard UI | 275, 1, 2 and 13 passed |
