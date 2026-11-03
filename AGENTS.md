# Repository Instructions

## Mission

Implement a small, independently deployable Soroban helper that atomically acquires BLNT V3 interest- or protocol-fee-auction bid tokens from USDC and fills the auction. A failed fill must roll back the caller's input transfer and every Comet swap or LP mint so that no newly acquired bid token remains in the caller's wallet.

This repository is operational infrastructure for BLNT V3. It is not part of the immutable lending protocol, does not modify pool economics, and must not introduce governance, custody beyond successful auction lots, or an alternate auction mechanism.

## Sources of truth

Read these before changing behavior:

1. `docs/SYSTEM_SPEC.md`
2. The pinned BLNT V3 system specification and deployed public APIs
3. The official Comet V1.1 ABI and release artifact
4. Existing tests and the implementation being changed

The deployed BLNT V3 pool, backstop, and canonical Comet contracts are external compatibility boundaries. Do not assume their code can be changed.

## Critical invariants

- Preserve atomic rollback across input transfer, exact-output swaps or LP acquisition, approval or direct BLNT payment, and auction fill.
- Never disable overflow checks or use floating-point arithmetic.
- Validate the live auction identity, bid token, fill percentage, and maximum input before moving value.
- Acquire exactly the execution-ledger bid amount. Do not add a bid-token buffer.
- Authorize the exact V3 donation amount for every possible private tier; the exact allowance must permit only one donation.
- Return unused caller input before invoking the fill.
- Only the configured owner may claim successful auction lots or transfer ownership.
- Do not add privileged control over BLNT V3 pools, backstops, auctions, or protocol state.
- Keep loops bounded. Interest-auction bid maps must contain exactly one token; claim lists are caller-bounded.
- Production WASM must include `source_repo=github:BLNT-Protocol/blnt-interest-auction-filler` metadata.

## Supported routes

The production contract supports every canonical BLNT V3 interest-tier token and protocol-fee auction from USDC:

- USDC directly;
- BLNT:USDC LP through an exact-output single-sided USDC deposit;
- XLM through exact-output USDC-to-BLNT and BLNT-to-XLM swaps;
- BLNT:XLM LP through exact-output USDC-to-BLNT acquisition and a single-sided BLNT deposit; and
- protocol-fee BLNT through an exact-output USDC-to-BLNT swap.

Reject noncanonical bid tokens. The owner-authorized caller is responsible for supplying a valid BLNT V3 pool, matching the V2 filler trust model. Do not add arbitrary caller-selected routing contracts.

## Development workflow

Before editing, inspect the worktree and identify the affected invariant. Contract changes require regression tests covering successful execution and rollback at the relevant boundary.

Run:

```bash
make check
make test
make build
```

Use `apply_patch` for authored changes. Do not commit build output, secrets, account keys, operator-specific deployment configuration, or generated local state. Shared release manifests and canonical public network bindings are allowed.
