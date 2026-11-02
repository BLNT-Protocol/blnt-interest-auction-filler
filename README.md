# BLNT V3 Interest and Protocol-Fee Auction Filler

An optional Soroban helper that atomically acquires BLNT V3 auction bid tokens from USDC and fills the auction. If the fill fails, every input transfer, swap, and LP acquisition rolls back.

The helper supports all four canonical interest-tier assets—USDC, XLM, BLNT:USDC LP, and BLNT:XLM LP—and BLNT bids for protocol-fee auctions. It uses the canonical BLNT:USDC and BLNT:XLM Comet V1.1 pools for exact-output acquisition, so the configured owner can operate from USDC without keeping tier-token or BLNT inventory.

See [the system specification](docs/SYSTEM_SPEC.md) for the normative behavior and safety boundaries.

## Build and test

```bash
make check
make test
make build
```

The optimized WASM is written to `target/wasm32v1-none/optimized/blnt_interest_auction_filler.wasm` with source-repository metadata.

## Provenance

The design is based on Script3's MIT-licensed [`interest-auction-filler`](https://github.com/script3/interest-auction-filler) for Blend V2 and is adapted to BLNT V3's tiered backstop and Protocol 27 interfaces. See [NOTICE.md](NOTICE.md).

## License

GNU Affero General Public License v3.0 only. Upstream MIT notices are preserved in [NOTICE.md](NOTICE.md).
