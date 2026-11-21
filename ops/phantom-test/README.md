# Real Phantom deposit test

Drives the Task Node's Solana deposit card with the real Phantom extension in Chromium, against
the real wallet server and mainnet. Paths are fixed to `/tmp/phantom-test`.

1. Extension: download the Phantom CRX (id `bfnaelmomeimhlpmgjnjophhpkkoljpa`) from the Chrome
   Web Store update URL, strip the CRX3 header and unzip to `/tmp/phantom-test/ext`.
2. Test wallet: `~/.config/postfiat/sol-bridge/phantom-test-wallet.json` (`Ce5Yv528…cfo8`). Copy it
   to `/tmp/phantom-test/test-wallet.json`. Keep about 0.005 SOL and some USDC in it.
3. Harness page: in the Task Node checkout,
   `set -a; . ~/.config/tasknode/navcoin-relay.env; set +a; node scripts/phantom-live-server.mjs`.
4. Set a local profile password in the mode-0600 file `/tmp/phantom-test/profile-password`.
   Run `node onboard.mjs` once on an empty `/tmp/phantom-test/profile` (imports the key, sets the
   password; it stops at Phantom's username step, which is fine).
5. `node drive.mjs 0.02 send` connects, sends 0.02 USDC and prints the card until it is credited.

From a datacenter address Phantom's own simulation and RPC servers answer 403, so its popup says
"Failed to simulate" and asks for an extra confirmation. The driver presses through it. Real users
on home connections get a normal approval. This is also why the card asks Phantom to sign only and
our server broadcasts.
