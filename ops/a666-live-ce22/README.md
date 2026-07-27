# a666 live-ce22 additive issuance

These requests create a new `a666` asset on `postfiat-wan-devnet-2` without
modifying or migrating `a651`.

- Asset id: `300bf48a63a94770b6e67817f88cd1abf77e7f592a061e15682d7fd9973260af4c2e631e32df3c2c402b7d2fe272a293`
- Issuer: `pffcb93d9f87a843a8aa34e1adf241f5d58143e81b`
- Reserve operator/attestor: `pfd0c86d9084915e1fefd22eab891806397d5a5937`
- Initial holder/subscriber: `pfab9b9228942e5c529633a13aa271d5297bec6353`
- Proof profile: the already-registered CONTROLLED six-leg, multi-fetch profile
  `f0318291c1251067c92f6360830acadf0a02f865f62c85af387dfb28bdff7edd555deadf9e253351f729d253ea76aae0`

The initial epoch deliberately declares zero circulation and zero verified net
assets. It establishes a $1.00 NAV unit for a fresh share class without
double-counting the reserve value already reported for legacy `a651`.
Subsequent a666 supply may arise only from the route's primary subscription
against real pfUSDC settlement value.

Each file is a separate certified round. This avoids relying on unproven
same-round dependency compression. All live operations use the deployed binary
copied byte-for-byte from validator 0 and the existing six-validator topology.

