# Protocol 30 RPC fixtures

The synthetic `signed_ms.xdr` and `empty_tx_set_ms.xdr` header entries match
the companion stellar-baselib fixtures: ledger 42, protocol 30,
`closeTime = 1700000000`, `closeTimeMs = 1700000000123`, union arms 3/4.
They exercise decoding, not network signature/hash validation.

Each `_meta.xdr` wraps the corresponding entry in `LedgerCloseMetaV2` (union
arm 2, ext 0), a generalized transaction set (arm 1) with zero previous hash
and no phases, empty processing/upgrades/SCP arrays, zero live-state size,
and no evicted keys. Bytes are assembled from the XDR wire layout rather
than round-tripped through the SDK under test.
