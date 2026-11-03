# Per-operator deployment

## Deployment model

The filler is a single-owner execution and custody helper, not a protocol-wide singleton. Each independent auctioneer normally deploys its own instance with its filler account as `owner`. Multiple bot processes may share one instance only when they intentionally share the same filler account.

The optimized WASM is content-addressed and only needs to be uploaded to a network once. Every operator can deploy a separate instance from the published WASM hash. The repository publishes the shared release in `releases/v1.0.0.json` and canonical mainnet bindings in `config/mainnet.json`; it does not publish a canonical contract ID.

## Prerequisites

- Stellar CLI 27.0.0.
- `jq`, `make`, and the pinned Rust toolchain.
- A funded Stellar CLI identity for transaction submission.
- The filler owner's public `G...` address. This should be the auctioneer bot's `fillerKeypair` account.

Keep signing secrets in Stellar CLI's identity store or another signer. Do not put a secret key in the repository, command line, deployment record, or `BLNT_FILLER_OWNER`.

## Verify the release

```bash
make release-verify
make mainnet-plan
```

Release verification rebuilds the optimized artifact, checks its size and SHA-256 hash, and confirms its embedded `source_repo` metadata. The plan prints the immutable constructor bindings without submitting a transaction.

## Deploy an instance

Configure a funded Stellar CLI identity as the transaction source and supply the bot filler account as owner:

```bash
BLNT_FILLER_SOURCE_ACCOUNT=my-deployer \
BLNT_FILLER_OWNER=G... \
make mainnet-deploy
```

Set `BLNT_FILLER_RPC_URL` to override the public RPC endpoint and `BLNT_FILLER_INCLUSION_FEE` to set a maximum inclusion fee in stroops. `BLNT_FILLER_SALT` may provide an explicit 64-character hexadecimal salt; otherwise the script generates one securely.

The script checks whether the released WASM is already installed. If not, it uploads the exact verified artifact, deploys an instance from its hash, verifies the deployed WASM and owner, and writes an ignored operator record under `deployments/`.

## Verify an existing instance

```bash
BLNT_FILLER_OWNER=G... \
BLNT_FILLER_CONTRACT=C... \
make mainnet-verify
```

Verification compares the instance's fetched WASM with the published release and checks `get_owner`. The contract deliberately does not expose its immutable configuration, so the constructor bindings and deployment transaction in the operator's record remain the evidence for those values. The auctioneer bot separately validates the owner, pool/backstop relationship, and canonical Comet token pairs before operating.

## Canonical mainnet constructor bindings

| Binding | Contract |
| --- | --- |
| Backstop | `CAHCUKBPTW5DR5RNXOHCTJD5U76A6EEZXYZAQFRQEEQRXQ3CRUZ6Z3MO` |
| BLNT | `CCBNWP5L7ZSXA3DBWCMFAVKFVRW3ZHWO7B5J557W42O64DDZNPJP7VDS` |
| USDC | `CCW67TSZV3SSS2HXMBQ5JFGCKJNXKZM7UQUWUZPUTHXSTZLEO7SJMI75` |
| Native XLM SAC | `CAS3J7GYLGXMF6TDJBBYYSE3HQ6BBSMLNUQ34T6TZMYMW2EVH34XOWMA` |
| BLNT:USDC Comet V1.1 | `CDQMIKIHKA5UY3FQJ2UDTLNX3GOPD3GPNZWSPTOBVOHEUMQJQXOPC7Y3` |
| BLNT:XLM Comet V1.1 | `CA5HX3D47PQZWC2E7VWXCKH4534P2NOJEOIXAWU47MI4QV2FQUYY3FLC` |
